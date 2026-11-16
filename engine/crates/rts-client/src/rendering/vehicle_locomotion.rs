use super::entity_articulation::{
    apply_wheel_spin, compute_wheel_spin_delta, discover_wheel_descendants, ArticulatedNodeFilter,
    DiscoveredWheelNode,
};
use super::preview_stage::PreviewStageEntity;
use super::{
    ArticulatedPartKind, BuildingCollapseVisual, FogOfWarPlaneEntity, PlacementGhostModel,
    SimVisualEntity, VehicleWreckVisual,
};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MovementDomain;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleLocomotionKind {
    GroundVehicle,
    FixedWingAircraft,
    HelicopterOrHover,
    NavalVessel,
}

/// Per-vehicle procedural chassis dynamics & wheel-spin tracker (`Deliverable 4`).
///
/// Convention for `nose_pitch_rad`:
/// - Positive (`> 0.0`): nose (`+Z`) pitches **UP** (e.g., `+3.5°` main-gun firing recoil rock, aircraft climb, chopper braking flare).
/// - Negative (`< 0.0`): nose (`+Z`) pitches **DOWN** (e.g., `-0.15` rad helicopter forward tilt at cruise speed, vehicle braking dive).
///
/// Converted to Bevy's right-handed `+X` rotation via `Quat::from_rotation_x(-nose_pitch_rad)`.
#[derive(Component, Debug, Clone)]
pub struct VehicleLocomotionState {
    pub kind: VehicleLocomotionKind,
    pub initialized: bool,
    pub prev_pos: Vec3,
    pub prev_yaw: f32,
    pub prev_speed_fwd: f32,
    pub prev_weapon_cooldown: f32,
    pub prev_recoil_timer: f32,
    pub nose_pitch_rad: f32,
    pub pitch_vel: f32,
    pub roll_rad: f32,
    pub roll_vel: f32,
    pub wheel_spin_rad: f32,
    pub wheels_discovered: bool,
    pub wheels: Vec<DiscoveredWheelNode>,
}

impl VehicleLocomotionState {
    pub fn new(kind: VehicleLocomotionKind) -> Self {
        Self {
            kind,
            initialized: false,
            prev_pos: Vec3::ZERO,
            prev_yaw: 0.0,
            prev_speed_fwd: 0.0,
            prev_weapon_cooldown: 0.0,
            prev_recoil_timer: 0.0,
            nose_pitch_rad: 0.0,
            pitch_vel: 0.0,
            roll_rad: 0.0,
            roll_vel: 0.0,
            wheel_spin_rad: 0.0,
            wheels_discovered: false,
            wheels: Vec::new(),
        }
    }
}

#[inline]
pub fn shortest_angle_diff(from_rad: f32, to_rad: f32) -> f32 {
    (to_rad - from_rad + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

pub fn classify_vehicle_locomotion_kind(
    loaded: &LoadedMod,
    unit_id: &str,
    has_rotor_node: bool,
) -> VehicleLocomotionKind {
    let low = unit_id.to_ascii_lowercase();
    if let Some(udef) = loaded.units.get(unit_id) {
        match udef.movement_domain {
            MovementDomain::Air => {
                let is_chopper_or_hover = has_rotor_node
                    || udef.visual.spin_speed_deg_per_sec.abs() > 20.0
                    || low.contains("chopper")
                    || low.contains("heli")
                    || low.contains("osprey")
                    || low.contains("disc")
                    || low.contains("ufo")
                    || low.contains("kirov")
                    || low.contains("airship");
                if is_chopper_or_hover {
                    return VehicleLocomotionKind::HelicopterOrHover;
                }
                return VehicleLocomotionKind::FixedWingAircraft;
            }
            MovementDomain::Naval => return VehicleLocomotionKind::NavalVessel,
            MovementDomain::Jumpjet => return VehicleLocomotionKind::HelicopterOrHover,
            MovementDomain::Amphibious => {
                if low.contains("hover") || low.contains("robot_tank") {
                    return VehicleLocomotionKind::HelicopterOrHover;
                }
                return VehicleLocomotionKind::GroundVehicle;
            }
            MovementDomain::Ground | MovementDomain::Chrono => {
                if low.contains("hover") {
                    return VehicleLocomotionKind::HelicopterOrHover;
                }
                return VehicleLocomotionKind::GroundVehicle;
            }
        }
    }
    if low.contains("harrier")
        || low.contains("eagle")
        || low.contains("plane")
        || low.contains("fighter")
    {
        VehicleLocomotionKind::FixedWingAircraft
    } else if has_rotor_node || low.contains("chopper") {
        VehicleLocomotionKind::HelicopterOrHover
    } else {
        VehicleLocomotionKind::GroundVehicle
    }
}

/// Evaluates one frame of `VehicleLocomotionState` kinematics for ground suspension & firing rock,
/// fixed-wing turn banking (`clamp(-yaw_rate * 0.52, -0.65, 0.65)`), helicopter forward tilt & brake flare,
/// and `+X` wheel rotation.
#[allow(clippy::too_many_arguments)]
pub fn step_vehicle_locomotion_sample(
    loco: &mut VehicleLocomotionState,
    dt: f32,
    curr_pos: Vec3,
    curr_yaw: f32,
    turret_yaw: f32,
    weapon_cooldown: f32,
    recoil_timer: f32,
    recoil_rock_deg: f32,
    max_speed_ref: f32,
    landed_at_airfield: bool,
) {
    let safe_dt = dt.clamp(1.0 / 240.0, 0.10);
    if !loco.initialized || curr_pos.distance_squared(loco.prev_pos) > 36.0 {
        loco.initialized = true;
        loco.prev_pos = curr_pos;
        loco.prev_yaw = curr_yaw;
        loco.prev_weapon_cooldown = weapon_cooldown;
        loco.prev_recoil_timer = recoil_timer;
        return;
    }

    let delta = curr_pos - loco.prev_pos;
    let fwd_x = curr_yaw.sin();
    let fwd_z = curr_yaw.cos();
    let signed_fwd_dist = delta.x * fwd_x + delta.z * fwd_z;
    let speed_fwd = (signed_fwd_dist / safe_dt).clamp(-28.0, 28.0);
    let yaw_delta = shortest_angle_diff(loco.prev_yaw, curr_yaw);
    let yaw_rate = (yaw_delta / safe_dt).clamp(-6.0, 6.0);
    let accel_fwd = ((speed_fwd - loco.prev_speed_fwd) / safe_dt).clamp(-22.0, 22.0);

    let fired_just_now = recoil_timer > loco.prev_recoil_timer + 0.02
        || weapon_cooldown > loco.prev_weapon_cooldown + 0.06;

    if !loco.wheels.is_empty() {
        let avg_r = loco.wheels[0].radius.max(0.18);
        let d_theta = compute_wheel_spin_delta(loco.prev_pos, curr_pos, curr_yaw, avg_r);
        loco.wheel_spin_rad = (loco.wheel_spin_rad + d_theta).rem_euclid(std::f32::consts::TAU);
    }

    match loco.kind {
        VehicleLocomotionKind::GroundVehicle | VehicleLocomotionKind::NavalVessel => {
            // Spring-damper chassis suspension (`omega = 11.5`, `zeta = 0.62`)
            let omega = if loco.kind == VehicleLocomotionKind::NavalVessel {
                7.0
            } else {
                11.5
            };
            let zeta = 0.62;

            // Acceleration squats rear (nose UP > 0); braking dips nose DOWN (< 0)
            let accel_pitch_target = (accel_fwd * 0.0042).clamp(-0.065, 0.065);
            // Centrifugal body roll on fast turns
            let turn_roll_target = (yaw_rate * speed_fwd * 0.0048).clamp(-0.075, 0.075);

            let pitch_err = loco.nose_pitch_rad - accel_pitch_target;
            let pitch_acc = -omega * omega * pitch_err - 2.0 * zeta * omega * loco.pitch_vel;
            loco.pitch_vel += pitch_acc * safe_dt;
            loco.nose_pitch_rad =
                (loco.nose_pitch_rad + loco.pitch_vel * safe_dt).clamp(-0.18, 0.18);

            let roll_err = loco.roll_rad - turn_roll_target;
            let roll_acc = -omega * omega * roll_err - 2.0 * zeta * omega * loco.roll_vel;
            loco.roll_vel += roll_acc * safe_dt;
            loco.roll_rad = (loco.roll_rad + loco.roll_vel * safe_dt).clamp(-0.18, 0.18);

            if fired_just_now {
                let rock_rad = recoil_rock_deg.clamp(2.8, 4.5).to_radians();
                let rel_turret = shortest_angle_diff(curr_yaw, turret_yaw);
                let pitch_kick = rock_rad * rel_turret.cos();
                let roll_kick = -rock_rad * rel_turret.sin() * 0.85;
                loco.nose_pitch_rad = (loco.nose_pitch_rad + pitch_kick * 0.82).clamp(-0.16, 0.16);
                loco.pitch_vel = pitch_kick * 9.5;
                loco.roll_rad = (loco.roll_rad + roll_kick * 0.82).clamp(-0.16, 0.16);
                loco.roll_vel = roll_kick * 8.5;
            }
        }
        VehicleLocomotionKind::FixedWingAircraft => {
            if landed_at_airfield {
                let settle = (10.0 * safe_dt).clamp(0.0, 1.0);
                loco.roll_rad += (0.0 - loco.roll_rad) * settle;
                loco.nose_pitch_rad += (0.0 - loco.nose_pitch_rad) * settle;
                loco.pitch_vel = 0.0;
                loco.roll_vel = 0.0;
            } else {
                let target_roll = (-yaw_rate * 0.52).clamp(-0.65, 0.65);
                let horiz_spd = Vec2::new(delta.x, delta.z).length() / safe_dt;
                let vy = (delta.y / safe_dt).clamp(-15.0, 15.0);
                let target_climb_pitch = (vy / horiz_spd.max(4.0)).atan().clamp(-0.42, 0.42);
                let bank_blend = (8.5 * safe_dt).clamp(0.02, 1.0);
                let pitch_blend = (7.0 * safe_dt).clamp(0.02, 1.0);
                loco.roll_rad += (target_roll - loco.roll_rad) * bank_blend;
                loco.nose_pitch_rad += (target_climb_pitch - loco.nose_pitch_rad) * pitch_blend;
            }
        }
        VehicleLocomotionKind::HelicopterOrHover => {
            if landed_at_airfield {
                let settle = (9.0 * safe_dt).clamp(0.0, 1.0);
                loco.nose_pitch_rad += (0.0 - loco.nose_pitch_rad) * settle;
                loco.roll_rad += (0.0 - loco.roll_rad) * settle;
            } else {
                let speed_norm = (speed_fwd / max_speed_ref.max(2.5)).clamp(-1.0, 1.0);
                // Moving forward at speed => `-0.15` rad nose-down pitch; braking to hover => positive nose-up flare!
                let decel_flare = if speed_fwd >= 0.0 && accel_fwd < -1.2 {
                    ((-accel_fwd - 1.2) * 0.026).clamp(0.0, 0.16)
                } else {
                    0.0
                };
                let target_pitch = (-0.15 * speed_norm + decel_flare).clamp(-0.22, 0.20);
                let target_roll = (-yaw_rate * 0.28).clamp(-0.34, 0.34);
                let blend = (7.5 * safe_dt).clamp(0.02, 1.0);
                loco.nose_pitch_rad += (target_pitch - loco.nose_pitch_rad) * blend;
                loco.roll_rad += (target_roll - loco.roll_rad) * blend;
            }
        }
    }

    loco.prev_pos = curr_pos;
    loco.prev_yaw = curr_yaw;
    loco.prev_speed_fwd = speed_fwd;
    loco.prev_weapon_cooldown = weapon_cooldown;
    loco.prev_recoil_timer = recoil_timer;
}

#[allow(clippy::type_complexity)]
pub fn step_vehicle_locomotion_and_wheels(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    mut vehicle_q: Query<
        (
            Entity,
            &SimVisualEntity,
            &mut Transform,
            &Visibility,
            Option<&mut VehicleLocomotionState>,
        ),
        (
            Without<BuildingCollapseVisual>,
            Without<VehicleWreckVisual>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    children_q: Query<&Children>,
    mut node_q: Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) {
    let (Some(mod_res), Some(sim_res)) = (mod_res, sim_res) else {
        return;
    };
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let dt = time.delta_secs();

    for (ent, vis, mut root_tf, root_vis, loco_opt) in &mut vehicle_q {
        if vis.is_infantry || vis.building_footprint.is_some() {
            continue;
        }
        let Some(sim_ent) = world.entities.get(&vis.sim_id) else {
            continue;
        };
        let has_rotor = vis
            .cached_articulated_parts
            .as_deref()
            .is_some_and(|parts| parts.iter().any(|p| p.kind == ArticulatedPartKind::Rotor));
        let kind = classify_vehicle_locomotion_kind(loaded, &vis.entity_type_id, has_rotor);

        let Some(mut loco) = loco_opt else {
            let mut init_state = VehicleLocomotionState::new(kind);
            init_state.initialized = true;
            init_state.prev_pos = root_tf.translation;
            init_state.prev_yaw = sim_ent.facing_radians;
            init_state.prev_weapon_cooldown = sim_ent.weapon_cooldown;
            init_state.prev_recoil_timer = sim_ent.recoil_timer;
            commands.entity(ent).insert(init_state);
            continue;
        };

        if loco.kind != kind {
            loco.kind = kind;
        }
        if !loco.wheels_discovered && loco.kind == VehicleLocomotionKind::GroundVehicle {
            if let Some(wheels) = discover_wheel_descendants(ent, &children_q, &node_q) {
                loco.wheels = wheels;
                loco.wheels_discovered = true;
            }
        }

        let udef_opt = loaded.units.get(&vis.entity_type_id);
        let max_speed_ref = udef_opt.map(|u| u.speed.max(2.5)).unwrap_or(6.0);
        let recoil_rock_deg = udef_opt
            .map(|u| {
                let d = u.visual.barrel_recoil_distance;
                if d >= 0.34 {
                    4.2
                } else if d >= 0.22 {
                    3.6
                } else {
                    3.0
                }
            })
            .unwrap_or(3.5);

        let (curr_yaw, _, _) = root_tf.rotation.to_euler(EulerRot::YXZ);
        let curr_pos = root_tf.translation;
        step_vehicle_locomotion_sample(
            &mut loco,
            dt,
            curr_pos,
            curr_yaw,
            sim_ent.turret_facing_radians,
            sim_ent.weapon_cooldown,
            sim_ent.recoil_timer,
            recoil_rock_deg,
            max_speed_ref,
            sim_ent.landed_at_airfield,
        );

        if *root_vis != Visibility::Hidden {
            let tilt_quat =
                Quat::from_euler(EulerRot::YXZ, 0.0, -loco.nose_pitch_rad, loco.roll_rad);
            let target_rot = (Quat::from_rotation_y(curr_yaw) * tilt_quat).normalize();
            if !root_tf.rotation.abs_diff_eq(target_rot, 1e-4)
                && !root_tf.rotation.abs_diff_eq(-target_rot, 1e-4)
            {
                root_tf.rotation = target_rot;
            }
            if !loco.wheels.is_empty() {
                apply_wheel_spin(&loco.wheels, loco.wheel_spin_rad, &mut node_q);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_vehicle_firing_recoil_rocks_chassis_between_2_8_and_4_5_deg_and_settles() {
        let mut loco = VehicleLocomotionState::new(VehicleLocomotionKind::GroundVehicle);
        step_vehicle_locomotion_sample(
            &mut loco,
            0.05,
            Vec3::new(10.0, 0.0, 10.0),
            0.0,
            0.0,
            0.0,
            0.0,
            3.8,
            6.0,
            false,
        );
        // Fire main cannon forward (`recoil_timer` jumps 0.0 -> 0.25)
        step_vehicle_locomotion_sample(
            &mut loco,
            0.05,
            Vec3::new(10.0, 0.0, 10.0),
            0.0,
            0.0,
            1.8,
            0.25,
            3.8,
            6.0,
            false,
        );
        let mut peak_deg = loco.nose_pitch_rad.to_degrees();
        for _ in 0..8 {
            step_vehicle_locomotion_sample(
                &mut loco,
                0.02,
                Vec3::new(10.0, 0.0, 10.0),
                0.0,
                0.0,
                1.6,
                0.15,
                3.8,
                6.0,
                false,
            );
            peak_deg = peak_deg.max(loco.nose_pitch_rad.to_degrees());
        }
        assert!(
            (2.8..=4.6).contains(&peak_deg),
            "expected firing chassis rock in 2.8..=4.5 deg, got {peak_deg:.2} deg"
        );

        // Settles back to ~0 deg over 1.2s
        for _ in 0..30 {
            step_vehicle_locomotion_sample(
                &mut loco,
                0.04,
                Vec3::new(10.0, 0.0, 10.0),
                0.0,
                0.0,
                0.0,
                0.0,
                3.8,
                6.0,
                false,
            );
        }
        assert!(loco.nose_pitch_rad.abs() < 0.01);
    }

    #[test]
    fn fixed_wing_aircraft_banks_into_turns_and_chopper_pitches_nose_down_then_flares() {
        let mut jet = VehicleLocomotionState::new(VehicleLocomotionKind::FixedWingAircraft);
        step_vehicle_locomotion_sample(
            &mut jet,
            0.05,
            Vec3::new(20.0, 5.2, 20.0),
            0.0,
            0.0,
            0.0,
            0.0,
            3.0,
            9.5,
            false,
        );
        // Steady left turn (`yaw_rate = +1.0 rad/s` => `target_roll = -0.52 rad`)
        let mut yaw = 0.0_f32;
        for _ in 0..20 {
            yaw += 0.05;
            step_vehicle_locomotion_sample(
                &mut jet,
                0.05,
                Vec3::new(20.0 + yaw.sin() * 4.0, 5.2, 20.0 + yaw.cos() * 4.0),
                yaw,
                yaw,
                0.0,
                0.0,
                3.0,
                9.5,
                false,
            );
        }
        assert!(
            (jet.roll_rad + 0.52).abs() < 0.04 && jet.roll_rad >= -0.65,
            "jet roll should converge to -0.52 rad, got {}",
            jet.roll_rad
        );

        // Helicopter moving forward at cruise speed (`7.8 m/s`) => `-0.15` rad nose-down pitch, then braking flare > 0
        let mut heli = VehicleLocomotionState::new(VehicleLocomotionKind::HelicopterOrHover);
        let mut z = 10.0_f32;
        for _ in 0..20 {
            z += 7.8 * 0.05;
            step_vehicle_locomotion_sample(
                &mut heli,
                0.05,
                Vec3::new(10.0, 4.6, z),
                0.0,
                0.0,
                0.0,
                0.0,
                3.0,
                7.8,
                false,
            );
        }
        assert!(
            (heli.nose_pitch_rad + 0.15).abs() < 0.02,
            "heli cruise pitch should converge to -0.15 rad, got {}",
            heli.nose_pitch_rad
        );

        // Abrupt brake to hover (`speed -> 0`) flares nose upward relative to `-0.15`
        step_vehicle_locomotion_sample(
            &mut heli,
            0.05,
            Vec3::new(10.0, 4.6, z),
            0.0,
            0.0,
            0.0,
            0.0,
            3.0,
            7.8,
            false,
        );
        assert!(heli.nose_pitch_rad > -0.13);
    }
}
