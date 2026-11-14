use crate::det_math::{
    det_atan2_scalar, det_distance_xz, det_sin_cos_scalar, det_sqrt, det_wrap_pi_scalar, SimScalar,
    OCTANT_UNIT_DIRS,
};
use crate::world::{EntityKind, SimEntity};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    ArmorType, BuildingDef, BuildingRole, LocomotorKind, MovementDomain, TeamMaskMode, UnitDef,
    UnitRole,
};

#[path = "locomotors_orca.rs"]
mod locomotors_orca;
pub use locomotors_orca::step_orca_avoidance;

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
    if u_def.visual.team_mask_mode == TeamMaskMode::AlphaMask
        && matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester)
        && u_def.health >= 260.0
    {
        return 2;
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

/// Computes the world-space beam corridor hit radius for `u_def` (`0.45..=1.8m`).
pub fn effective_unit_beam_radius(u_def: &UnitDef) -> f32 {
    let raw_xz = if u_def.visual.team_mask_mode == TeamMaskMode::AlphaMask {
        match u_def.role {
            UnitRole::Vehicle | UnitRole::Harvester if u_def.health >= 260.0 => 2.2,
            _ => 0.95,
        }
    } else {
        u_def.visual.scale[0].max(u_def.visual.scale[2])
    };
    (raw_xz * 0.5).clamp(0.45, 1.8)
}

/// Computes the vertical Line-Of-Sight / firing eye elevation bonus for tall tower buildings & landmarks.
pub fn effective_building_tower_bonus(b_def: &BuildingDef) -> f32 {
    if b_def.visual.scale[1] >= 2.5 {
        return b_def.visual.scale[1];
    }
    if b_def.visual.team_mask_mode == TeamMaskMode::AlphaMask {
        let max_fp = b_def.footprint[0].max(b_def.footprint[1]);
        let is_landmark_tower =
            !b_def.visual.team_color_tint && max_fp >= 2 && b_def.vision_range >= 20.0;
        let is_super_beacon = b_def.role == BuildingRole::TechCenter && b_def.vision_range >= 24.0;
        if is_landmark_tower || is_super_beacon {
            return 4.4;
        }
    }
    0.0
}

/// Computes the 3D ballistic occlusion roof height (`1.85..=5.0m`) for a `BuildingDef`.
pub fn effective_building_roof_height(b_def: &BuildingDef) -> f32 {
    match b_def.role {
        BuildingRole::Wall | BuildingRole::Gate => 1.85_f32,
        BuildingRole::BridgeRepairHut => 2.4_f32,
        _ if b_def.visual.team_mask_mode == TeamMaskMode::AlphaMask => {
            let max_fp = b_def.footprint[0].max(b_def.footprint[1]);
            match b_def.role {
                BuildingRole::TechCenter => 5.0,
                BuildingRole::ConstructionYard
                | BuildingRole::WarFactory
                | BuildingRole::Airfield => 4.32,
                BuildingRole::PowerPlant if max_fp >= 3 => 5.0,
                BuildingRole::PowerPlant | BuildingRole::Refinery | BuildingRole::ServiceDepot => {
                    3.84
                }
                BuildingRole::Barracks => 3.36,
                BuildingRole::Defense if max_fp == 1 && b_def.cost >= 1000 => 4.40,
                BuildingRole::Defense if max_fp == 1 => 2.88,
                _ => ((max_fp as f32) * 1.44).clamp(2.88, 5.0),
            }
        }
        _ => (b_def.visual.scale[1] * 2.4).clamp(2.4, 5.0),
    }
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
