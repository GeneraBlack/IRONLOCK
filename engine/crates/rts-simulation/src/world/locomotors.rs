use crate::det_math::{
    det_atan2_scalar, det_distance_xz, det_sin_cos_scalar, det_sqrt, det_wrap_pi_scalar, SimScalar,
    OCTANT_UNIT_DIRS,
};
use crate::world::{EntityKind, SimEntity, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ArmorType, LocomotorKind, MovementDomain, UnitDef, UnitRole};
use std::collections::BTreeMap;

/// Default turret alignment tolerance (`15 deg` in radians) required before a turreted unit fires.
pub const TURRET_FIRING_TOLERANCE_RAD: f32 = 0.261_799_4;

/// Returns `true` when `u_def` uses an independent rotating turret (`visual.has_turret`,
/// `visual.archetype == Tank | Turret`, or explicit `locomotor.turret_slew_deg_per_sec > 0.0`).
pub fn unit_has_turret(u_def: &UnitDef) -> bool {
    u_def.visual.has_turret
        || u_def.locomotor.turret_slew_deg_per_sec > 0.0
        || matches!(
            u_def.visual.archetype,
            rts_sdk::schema::VisualArchetype::Tank | rts_sdk::schema::VisualArchetype::Turret
        )
}

/// Returns `true` when `u_def` explicitly enforces turret slew alignment tolerance before firing
/// (`locomotor.turret_slew_deg_per_sec > 0.0` and non-kamikaze).
pub fn unit_gates_fire_on_turret(u_def: &UnitDef) -> bool {
    u_def.locomotor.turret_slew_deg_per_sec > 0.0 && !u_def.kamikaze
}

/// Computes the tile clearance diameter (`1`, `2`, or `3` tiles) required by `ent` when pathing.
pub fn unit_clearance_tiles(ent: &SimEntity, loaded: &LoadedMod) -> u8 {
    if let Some([w, h]) = ent.custom_footprint {
        return w.max(h).clamp(1, 3) as u8;
    }
    match &ent.kind {
        EntityKind::Unit { unit_id } => loaded
            .units
            .get(unit_id)
            .map(unit_def_clearance_tiles)
            .unwrap_or(1),
        EntityKind::Building { building_id } => loaded
            .buildings
            .get(building_id)
            .map(|b| b.footprint[0].max(b.footprint[1]).clamp(1, 3) as u8)
            .unwrap_or(2),
    }
}

/// Computes the tile clearance diameter (`1..=3`) for a `UnitDef`.
pub fn unit_def_clearance_tiles(u_def: &UnitDef) -> u8 {
    if matches!(
        u_def.movement_domain,
        MovementDomain::Air | MovementDomain::Jumpjet
    ) || u_def.subterranean
    {
        return 1;
    }
    let max_scale = u_def.visual.scale[0].max(u_def.visual.scale[2]);
    if max_scale >= 1.85 {
        return 2;
    }
    if u_def.movement_domain == MovementDomain::Naval && u_def.health >= 500.0 {
        return 2;
    }
    if u_def.role == UnitRole::Vehicle
        && u_def.armor == ArmorType::Heavy
        && u_def.health >= 650.0
        && (u_def.crushes_vehicles || max_scale >= 1.55)
    {
        return 2;
    }
    1
}

/// Deterministically slews `current_rad` toward `target_rad` by at most `max_step_rad` using
/// fixed-point `SimScalar` angle wrapping. Returns `(new_angle_rad, remaining_abs_error_rad)`.
pub fn slew_angle_towards(current_rad: f32, target_rad: f32, max_step_rad: f32) -> (f32, f32) {
    let cur = SimScalar::from_f32(current_rad);
    let tgt = SimScalar::from_f32(target_rad);
    let max_step = SimScalar::from_f32(max_step_rad.max(0.0));
    let diff = det_wrap_pi_scalar(tgt - cur);
    let abs_diff = diff.abs();
    if abs_diff <= max_step {
        (det_wrap_pi_scalar(tgt).to_f32(), 0.0)
    } else {
        let step = if diff.is_negative() {
            -max_step
        } else {
            max_step
        };
        let next = det_wrap_pi_scalar(cur + step);
        let rem = (abs_diff - max_step).max(SimScalar::ZERO);
        (next.to_f32(), rem.to_f32())
    }
}

/// Computes the effective speed multiplier (`[0.25, 1.0]`) caused by infantry suppression.
pub fn unit_suppression_speed_multiplier(ent: &SimEntity, u_def: &UnitDef) -> f32 {
    if ent.suppression <= 0.0 {
        return 1.0;
    }
    if let Some(ref sq) = u_def.squad_trait {
        sq.suppression_efficiency(ent.suppression)
    } else if matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer) {
        let p = (ent.suppression / 100.0).clamp(0.0, 1.0);
        (1.0 - 0.55 * p).clamp(0.35, 1.0)
    } else {
        1.0
    }
}

/// Advances idle speed deceleration or continuous `FixedWingAircraft` combat attack-run / holding flight
/// when `ent.path_waypoints` is empty. Returns `true` if `ent.position` changed.
pub fn step_non_waypoint_locomotor(
    ent: &mut SimEntity,
    u_def: &UnitDef,
    eff_max_speed: f32,
    attack_focus_pos: Option<[f32; 3]>,
    near_friendly_airfield: bool,
    dt: f32,
) -> bool {
    let spec = u_def.locomotor_spec();
    let kind = spec.kind.unwrap_or(LocomotorKind::Infantry);
    let dt_s = SimScalar::from_f32(dt);

    if kind == LocomotorKind::FixedWingAircraft
        && !ent.landed_at_airfield
        && !(ent.script_ctx.deployed && u_def.lands_when_deployed)
        && eff_max_speed > 0.0
    {
        let should_attack_run =
            attack_focus_pos.is_some() && (ent.max_ammo == 0 || ent.current_ammo > 0);
        let should_hold_orbit =
            !should_attack_run && !near_friendly_airfield && ent.current_speed > 0.25;

        if should_attack_run || should_hold_orbit {
            let max_spd = SimScalar::from_f32(eff_max_speed);
            let accel = SimScalar::from_f32(spec.accel.max(4.0));
            let cur_spd =
                SimScalar::from_f32(ent.current_speed).max(max_spd * SimScalar::from_f32(0.55));
            let next_spd = (cur_spd + accel * dt_s).min(max_spd);
            ent.current_speed = next_spd.to_f32();

            let run_radius = spec.attack_run_radius.max(5.5);
            let steer_goal = if let Some(focus) = attack_focus_pos {
                let dist_to_target = det_distance_xz(ent.position, focus).to_f32();
                // Fly outbound along loiter_angle while weapon is reloading or right after passing over target;
                // bank back toward target once outside inner turn-around threshold and weapon is ready/nearly ready.
                let outbound = ent.weapon_cooldown > 0.25 || dist_to_target < 2.2;
                if outbound {
                    let (sin_a, cos_a) = det_sin_cos_scalar(SimScalar::from_f32(ent.loiter_angle));
                    let r = SimScalar::from_f32(run_radius);
                    [
                        (SimScalar::from_f32(focus[0]) + cos_a * r).to_f32(),
                        ent.position[1],
                        (SimScalar::from_f32(focus[2]) + sin_a * r).to_f32(),
                    ]
                } else {
                    focus
                }
            } else {
                let anchor = ent.guard_origin.unwrap_or(ent.position);
                let (sin_a, cos_a) = det_sin_cos_scalar(SimScalar::from_f32(ent.loiter_angle));
                let r = SimScalar::from_f32((run_radius * 0.45).clamp(2.4, 4.8));
                [
                    (SimScalar::from_f32(anchor[0]) + cos_a * r).to_f32(),
                    ent.position[1],
                    (SimScalar::from_f32(anchor[2]) + sin_a * r).to_f32(),
                ]
            };

            let dx = SimScalar::from_f32(steer_goal[0]) - SimScalar::from_f32(ent.position[0]);
            let dz = SimScalar::from_f32(steer_goal[2]) - SimScalar::from_f32(ent.position[2]);
            let desired_yaw = det_atan2_scalar(dx, dz);
            let max_turn_step = SimScalar::from_f32(spec.turn_rate_deg_per_sec.to_radians()) * dt_s;
            let (new_yaw, _) = slew_angle_towards(
                ent.facing_radians,
                desired_yaw.to_f32(),
                max_turn_step.to_f32(),
            );
            ent.facing_radians = new_yaw;
            if ent.target_entity_id.is_none() {
                ent.turret_facing_radians = new_yaw;
            }

            let (fwd_x, fwd_z) = det_sin_cos_scalar(SimScalar::from_f32(ent.facing_radians));
            let step = next_spd * dt_s;
            ent.position[0] = (SimScalar::from_f32(ent.position[0]) + fwd_x * step).to_f32();
            ent.position[2] = (SimScalar::from_f32(ent.position[2]) + fwd_z * step).to_f32();
            return true;
        }
    }

    // Decelerate non-moving units smoothly down to zero speed
    if ent.current_speed > 0.0 {
        let decel = SimScalar::from_f32(spec.decel.max(8.0));
        let cur = SimScalar::from_f32(ent.current_speed);
        let next = (cur - decel * dt_s).max(SimScalar::ZERO);
        ent.current_speed = next.to_f32();
    }
    false
}

/// Steps deterministic fixed-point `Locomotor` chassis turning, pivot-in-place, acceleration/deceleration
/// ramps, and waypoint progression for a unit with active `path_waypoints`.
pub fn step_waypoint_locomotor(ent: &mut SimEntity, u_def: &UnitDef, eff_max_speed: f32, dt: f32) {
    if ent.path_waypoints.is_empty() {
        return;
    }
    let spec = u_def.locomotor_spec();
    let kind = spec.kind.unwrap_or(LocomotorKind::Infantry);
    let dt_s = SimScalar::from_f32(dt);
    let max_spd = SimScalar::from_f32(eff_max_speed.max(0.0));
    if max_spd <= SimScalar::ZERO {
        ent.current_speed = 0.0;
        return;
    }

    let next_wp = ent.path_waypoints[0];
    let pos_x = SimScalar::from_f32(ent.position[0]);
    let pos_z = SimScalar::from_f32(ent.position[2]);
    let wp_x = SimScalar::from_f32(next_wp[0]);
    let wp_z = SimScalar::from_f32(next_wp[2]);
    let dx = wp_x - pos_x;
    let dz = wp_z - pos_z;
    let dist = det_sqrt(dx * dx + dz * dz);

    let is_final_wp = ent.path_waypoints.len() == 1 && ent.active_flow_handle.is_none();
    let explicit_turn = u_def.locomotor.turn_rate_deg_per_sec > 0.0;
    let turn_rate_rad = spec.turn_rate_deg_per_sec.max(30.0).to_radians();
    let pivot_threshold_rad = if explicit_turn {
        65.0_f32.to_radians()
    } else {
        95.0_f32.to_radians()
    };

    let abs_yaw_err = if dist > SimScalar::from_f32(0.001) {
        let raw_desired = if ent.reverse_moving {
            det_atan2_scalar(-dx, -dz)
        } else {
            det_atan2_scalar(dx, dz)
        };
        let eff_turn_rad = if kind == LocomotorKind::Tracked && ent.current_speed < 0.25 {
            if explicit_turn {
                turn_rate_rad * 1.45
            } else {
                360.0_f32.to_radians().max(turn_rate_rad * 1.5)
            }
        } else {
            turn_rate_rad
        };
        let (new_facing, rem_err) =
            slew_angle_towards(ent.facing_radians, raw_desired.to_f32(), eff_turn_rad * dt);
        ent.facing_radians = new_facing;
        if ent.target_entity_id.is_none() && ent.attack_ground_pos.is_none() {
            let max_slew = spec
                .turret_slew_deg_per_sec
                .max(eff_turn_rad.to_degrees())
                .to_radians()
                * dt;
            let (new_turret, _) =
                slew_angle_towards(ent.turret_facing_radians, new_facing, max_slew);
            ent.turret_facing_radians = new_turret;
        }
        rem_err
    } else {
        0.0
    };

    // Tracked vehicles pivot in place before driving when facing error exceeds pivot threshold
    if kind == LocomotorKind::Tracked
        && abs_yaw_err > pivot_threshold_rad
        && dist > SimScalar::from_f32(0.85)
    {
        let decel = SimScalar::from_f32(spec.decel.max(10.0));
        let next_spd = (SimScalar::from_f32(ent.current_speed) - decel * dt_s).max(SimScalar::ZERO);
        ent.current_speed = next_spd.to_f32();
        return;
    }

    // Target speed throttle based on locomotor turn alignment & final-waypoint braking distance
    let mut target_spd = max_spd;
    match kind {
        LocomotorKind::Wheeled if abs_yaw_err > 0.35 => {
            let factor = (1.0 - (abs_yaw_err / std::f32::consts::PI) * 0.55).clamp(0.45, 1.0);
            target_spd = max_spd * SimScalar::from_f32(factor);
        }
        LocomotorKind::Naval if abs_yaw_err > 0.45 => {
            let factor = (1.0 - (abs_yaw_err / std::f32::consts::PI) * 0.50).clamp(0.40, 1.0);
            target_spd = max_spd * SimScalar::from_f32(factor);
        }
        LocomotorKind::FixedWingAircraft => {
            // Fixed-wing jets never brake to zero at waypoints
            target_spd = max_spd;
        }
        _ => {}
    }

    let apply_braking = is_final_wp
        && kind != LocomotorKind::FixedWingAircraft
        && (u_def.locomotor.decel > 0.0 || dist < SimScalar::from_f32(0.75));
    if apply_braking {
        let decel_f = spec.decel.max(6.0);
        // v_stop = sqrt(2 * decel * dist)
        let two_a_d = SimScalar::from_f32(2.0 * decel_f) * dist;
        let braking_spd = det_sqrt(two_a_d).max(max_spd * SimScalar::from_f32(0.25));
        if braking_spd < target_spd {
            target_spd = braking_spd;
        }
    }

    let cur_spd = SimScalar::from_f32(ent.current_speed);
    let eff_accel = if u_def.locomotor.accel > 0.0 {
        spec.accel.max(1.0)
    } else {
        (eff_max_speed * 15.0).max(spec.accel)
    };
    let next_spd = if cur_spd < target_spd {
        let accel = SimScalar::from_f32(eff_accel);
        (cur_spd + accel * dt_s).min(target_spd)
    } else {
        let decel = SimScalar::from_f32(spec.decel.max(6.0));
        (cur_spd - decel * dt_s).max(target_spd)
    };
    let next_spd = if kind == LocomotorKind::FixedWingAircraft {
        next_spd.max(max_spd * SimScalar::from_f32(0.55))
    } else {
        next_spd
    };
    ent.current_speed = next_spd.to_f32();

    let step_dist = next_spd * dt_s;
    let passed_intermediate_wp = if !is_final_wp && ent.path_waypoints.len() > 1 {
        let wp1 = ent.path_waypoints[1];
        let seg_x = SimScalar::from_f32(wp1[0]) - wp_x;
        let seg_z = SimScalar::from_f32(wp1[2]) - wp_z;
        let rel_x = pos_x - wp_x;
        let rel_z = pos_z - wp_z;
        dist <= step_dist.max(SimScalar::from_f32(1.45))
            || (dist <= SimScalar::from_f32(2.40)
                && (rel_x * seg_x + rel_z * seg_z) >= SimScalar::ZERO)
    } else {
        false
    };
    if passed_intermediate_wp {
        ent.path_waypoints.remove(0);
    }

    let next_wp = ent.path_waypoints[0];
    let wp_x = SimScalar::from_f32(next_wp[0]);
    let wp_z = SimScalar::from_f32(next_wp[2]);
    let dx = wp_x - pos_x;
    let dz = wp_z - pos_z;
    let dist = det_sqrt(dx * dx + dz * dz);
    let is_final_wp = ent.path_waypoints.len() == 1 && ent.active_flow_handle.is_none();

    let arrival_radius = if kind == LocomotorKind::FixedWingAircraft {
        step_dist.max(SimScalar::from_f32(0.85))
    } else {
        step_dist
    };

    if is_final_wp && (dist <= arrival_radius || dist <= SimScalar::from_f32(0.02)) {
        if kind == LocomotorKind::FixedWingAircraft {
            let (fwd_x, fwd_z) = det_sin_cos_scalar(SimScalar::from_f32(ent.facing_radians));
            ent.position[0] = (pos_x + fwd_x * step_dist).to_f32();
            ent.position[2] = (pos_z + fwd_z * step_dist).to_f32();
        } else {
            ent.position[0] = wp_x.to_f32();
            ent.position[2] = wp_z.to_f32();
        }
        ent.path_waypoints.remove(0);
        if ent.path_waypoints.is_empty() {
            ent.reverse_moving = false;
            ent.formation_speed_cap = None;
            if kind != LocomotorKind::FixedWingAircraft {
                ent.current_speed = 0.0;
            }
        }
        return;
    }

    if dist <= SimScalar::from_f32(0.001) {
        return;
    }

    // Velocity direction: Infantry moves immediately along target vector; vehicles/ships/aircraft
    // advance along chassis heading (blending smoothly onto direct waypoint vector inside their
    // minimum turn diameter so finite-turn-radius vehicles never orbit 2m grid waypoints).
    let inv_dist = SimScalar::ONE / dist;
    let dir_x = dx * inv_dist;
    let dir_z = dz * inv_dist;
    let min_turn_diam =
        SimScalar::from_f32((2.4 * eff_max_speed / turn_rate_rad.max(0.5)).clamp(1.2, 6.0));

    let (move_x, move_z) = match kind {
        LocomotorKind::Infantry | LocomotorKind::Vtol => (dir_x, dir_z),
        LocomotorKind::Hover => {
            let sign = if ent.reverse_moving {
                -SimScalar::ONE
            } else {
                SimScalar::ONE
            };
            let (sin_f, cos_f) = det_sin_cos_scalar(SimScalar::from_f32(ent.facing_radians));
            let hx = sin_f * sign;
            let hz = cos_f * sign;
            let h_w = if dist < min_turn_diam {
                (dist / min_turn_diam) * SimScalar::from_f32(0.45)
            } else {
                SimScalar::from_f32(0.70)
            };
            let d_w = SimScalar::ONE - h_w;
            let bx = hx * h_w + dir_x * d_w;
            let bz = hz * h_w + dir_z * d_w;
            let blen = det_sqrt(bx * bx + bz * bz);
            if blen > SimScalar::from_f32(0.001) {
                (bx / blen, bz / blen)
            } else {
                (dir_x, dir_z)
            }
        }
        LocomotorKind::FixedWingAircraft => {
            let (sin_f, cos_f) = det_sin_cos_scalar(SimScalar::from_f32(ent.facing_radians));
            (sin_f, cos_f)
        }
        LocomotorKind::Wheeled | LocomotorKind::Tracked | LocomotorKind::Naval => {
            let sign = if ent.reverse_moving {
                -SimScalar::ONE
            } else {
                SimScalar::ONE
            };
            let (sin_f, cos_f) = det_sin_cos_scalar(SimScalar::from_f32(ent.facing_radians));
            let hx = sin_f * sign;
            let hz = cos_f * sign;
            if dist < min_turn_diam {
                let h_w = (dist / min_turn_diam) * SimScalar::from_f32(0.55);
                let d_w = SimScalar::ONE - h_w;
                let bx = hx * h_w + dir_x * d_w;
                let bz = hz * h_w + dir_z * d_w;
                let blen = det_sqrt(bx * bx + bz * bz);
                if blen > SimScalar::from_f32(0.001) {
                    (bx / blen, bz / blen)
                } else {
                    (dir_x, dir_z)
                }
            } else {
                (hx, hz)
            }
        }
    };

    ent.position[0] = (pos_x + move_x * step_dist).to_f32();
    ent.position[2] = (pos_z + move_z * step_dist).to_f32();
}

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
