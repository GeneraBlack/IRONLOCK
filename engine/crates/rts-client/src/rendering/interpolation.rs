use crate::SimWorldResource;
use bevy::prelude::*;
use rts_sdk::schema::DeliveryKind;
use rts_simulation::world::{EntityKind, SimEntity, SimWorld};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityTransformSample {
    pub position: Vec3,
    pub facing_radians: f32,
    pub turret_facing_radians: f32,
    pub recoil_timer: f32,
    pub current_speed: f32,
    pub teleport_epoch: u32,
    pub rotor_angle_rad: f32,
}

impl EntityTransformSample {
    pub fn from_sim_entity(ent: &SimEntity, prev_rotor_angle: Option<f32>, fixed_dt: f32) -> Self {
        let rotor_speed = match ent.kind {
            EntityKind::Unit { .. } => 14.5,
            EntityKind::Building { .. } => 2.2,
        };
        let next_rotor = prev_rotor_angle
            .map(|r| (r + rotor_speed * fixed_dt).rem_euclid(std::f32::consts::TAU))
            .unwrap_or_else(|| ((ent.id as f32) * 0.5).rem_euclid(std::f32::consts::TAU));

        Self {
            position: Vec3::from_array(ent.position),
            facing_radians: ent.facing_radians,
            turret_facing_radians: ent.turret_facing_radians,
            recoil_timer: ent.recoil_timer,
            current_speed: ent.current_speed,
            teleport_epoch: ent.teleport_epoch,
            rotor_angle_rad: next_rotor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectileTrackKey {
    pub attacker_player: u8,
    pub start_mm: [i32; 3],
    pub ordinal: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileTransformSample {
    pub key: ProjectileTrackKey,
    pub attacker_player: u8,
    pub start_pos: Vec3,
    pub current_pos: Vec3,
    pub target_pos: Vec3,
    pub velocity: Vec3,
    pub delivery: DeliveryKind,
    pub health: f32,
    pub max_health: f32,
    pub splash_radius: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SimTickSnapshot {
    pub tick: u64,
    pub initialized: bool,
    pub entities: HashMap<u64, EntityTransformSample>,
    pub projectiles: Vec<ProjectileTransformSample>,
}

impl SimTickSnapshot {
    pub fn capture(world: &SimWorld, prev: Option<&SimTickSnapshot>, fixed_dt: f32) -> Self {
        let mut entities = HashMap::with_capacity(world.entities.len());
        for (&id, ent) in &world.entities {
            let prev_rotor = prev
                .and_then(|p| p.entities.get(&id))
                .map(|s| s.rotor_angle_rad);
            entities.insert(
                id,
                EntityTransformSample::from_sim_entity(ent, prev_rotor, fixed_dt),
            );
        }

        let mut ordinal_counts: HashMap<(u8, [i32; 3]), u16> = HashMap::new();
        let mut projectiles = Vec::with_capacity(world.projectiles.len());
        for proj in &world.projectiles {
            let start_mm = [
                (proj.start_pos[0] * 100.0).round() as i32,
                (proj.start_pos[1] * 100.0).round() as i32,
                (proj.start_pos[2] * 100.0).round() as i32,
            ];
            let base = (proj.attacker_player, start_mm);
            let ord = ordinal_counts.entry(base).or_insert(0);
            let key = ProjectileTrackKey {
                attacker_player: proj.attacker_player,
                start_mm,
                ordinal: *ord,
            };
            *ord = ord.saturating_add(1);

            projectiles.push(ProjectileTransformSample {
                key,
                attacker_player: proj.attacker_player,
                start_pos: Vec3::from_array(proj.start_pos),
                current_pos: Vec3::from_array(proj.current_pos),
                target_pos: Vec3::from_array(proj.target_pos),
                velocity: Vec3::from_array(proj.velocity),
                delivery: proj.delivery,
                health: proj.health,
                max_health: proj.max_health,
                splash_radius: proj.splash_radius,
            });
        }

        Self {
            tick: world.tick,
            initialized: true,
            entities,
            projectiles,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct PrevSimSnapshot(pub SimTickSnapshot);

#[derive(Resource, Debug, Clone, Default)]
pub struct CurrSimSnapshot(pub SimTickSnapshot);

#[derive(Debug, Clone, Copy)]
pub struct InterpolatedEntityPose {
    pub position: Vec3,
    pub chassis_yaw_rot: Quat,
    pub facing_radians: f32,
    pub turret_facing_radians: f32,
    pub recoil_timer: f32,
    pub current_speed: f32,
    pub rotor_angle_rad: f32,
    pub snapped_teleport: bool,
}

#[derive(Debug, Clone)]
pub struct InterpolatedProjectilePose {
    pub attacker_player: u8,
    pub start_pos: Vec3,
    pub position: Vec3,
    pub target_pos: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub delivery: DeliveryKind,
    pub health: f32,
    pub max_health: f32,
    pub splash_radius: f32,
}

/// Shortest-arc angular interpolation in `[-PI, PI]` with exact wrap-around across `-PI <-> +PI`.
#[inline]
pub fn lerp_angle_shortest(a: f32, b: f32, alpha: f32) -> f32 {
    let t = alpha.clamp(0.0, 1.0);
    let delta =
        (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let raw = a + delta * t;
    (raw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Slerps two Y-axis yaw angles using `Quat::slerp`, taking the shortest geodesic arc.
#[inline]
pub fn slerp_yaw_quat(a_yaw: f32, b_yaw: f32, alpha: f32) -> Quat {
    let qa = Quat::from_rotation_y(a_yaw);
    let qb = Quat::from_rotation_y(b_yaw);
    qa.slerp(qb, alpha.clamp(0.0, 1.0)).normalize()
}

/// Interpolates `Prev -> Curr` entity transform state for `alpha = Time<Fixed>::overstep_fraction()`.
/// Immediately snaps without blending when `teleport_epoch` changes (Chronoshift, spawn, unload).
pub fn interpolate_entity_sample(
    prev: Option<&EntityTransformSample>,
    curr: &EntityTransformSample,
    alpha: f32,
) -> InterpolatedEntityPose {
    let t = alpha.clamp(0.0, 1.0);
    let Some(p) = prev else {
        return InterpolatedEntityPose {
            position: curr.position,
            chassis_yaw_rot: Quat::from_rotation_y(curr.facing_radians),
            facing_radians: curr.facing_radians,
            turret_facing_radians: curr.turret_facing_radians,
            recoil_timer: curr.recoil_timer,
            current_speed: curr.current_speed,
            rotor_angle_rad: curr.rotor_angle_rad,
            snapped_teleport: true,
        };
    };

    if p.teleport_epoch != curr.teleport_epoch {
        return InterpolatedEntityPose {
            position: curr.position,
            chassis_yaw_rot: Quat::from_rotation_y(curr.facing_radians),
            facing_radians: curr.facing_radians,
            turret_facing_radians: curr.turret_facing_radians,
            recoil_timer: curr.recoil_timer,
            current_speed: curr.current_speed,
            rotor_angle_rad: curr.rotor_angle_rad,
            snapped_teleport: true,
        };
    }

    let position = p.position.lerp(curr.position, t);
    let chassis_yaw_rot = slerp_yaw_quat(p.facing_radians, curr.facing_radians, t);
    let facing_radians = lerp_angle_shortest(p.facing_radians, curr.facing_radians, t);
    let turret_facing_radians =
        lerp_angle_shortest(p.turret_facing_radians, curr.turret_facing_radians, t);
    let recoil_timer = p.recoil_timer + (curr.recoil_timer - p.recoil_timer) * t;
    let current_speed = p.current_speed + (curr.current_speed - p.current_speed) * t;
    let rotor_angle_rad = lerp_angle_shortest(p.rotor_angle_rad, curr.rotor_angle_rad, t);

    InterpolatedEntityPose {
        position,
        chassis_yaw_rot,
        facing_radians,
        turret_facing_radians,
        recoil_timer,
        current_speed,
        rotor_angle_rad,
        snapped_teleport: false,
    }
}

/// Interpolates all active `SimProjectile` trajectories between `PrevSimSnapshot` and `CurrSimSnapshot`.
pub fn interpolate_projectiles(
    prev: &SimTickSnapshot,
    curr: &SimTickSnapshot,
    alpha: f32,
) -> Vec<InterpolatedProjectilePose> {
    let t = alpha.clamp(0.0, 1.0);
    let prev_map: HashMap<ProjectileTrackKey, &ProjectileTransformSample> =
        prev.projectiles.iter().map(|p| (p.key, p)).collect();

    curr.projectiles
        .iter()
        .map(|c| {
            let (pos, vel) = if let Some(p) = prev_map.get(&c.key) {
                if p.current_pos.distance_squared(c.current_pos) <= 36.0 {
                    (
                        p.current_pos.lerp(c.current_pos, t),
                        p.velocity.lerp(c.velocity, t),
                    )
                } else {
                    (c.current_pos, c.velocity)
                }
            } else {
                // Newly spawned projectile on this tick: interpolate smoothly from muzzle/start_pos
                (c.start_pos.lerp(c.current_pos, t), c.velocity)
            };

            let dir = if vel.length_squared() > 1e-4 {
                vel.normalize()
            } else {
                (c.target_pos - c.start_pos).normalize_or_zero()
            };
            let rotation = if dir.length_squared() > 1e-4 {
                Quat::from_rotation_arc(Vec3::Z, dir)
            } else {
                Quat::IDENTITY
            };

            InterpolatedProjectilePose {
                attacker_player: c.attacker_player,
                start_pos: c.start_pos,
                position: pos,
                target_pos: c.target_pos,
                velocity: vel,
                rotation,
                delivery: c.delivery,
                health: c.health,
                max_health: c.max_health,
                splash_radius: c.splash_radius,
            }
        })
        .collect()
}

/// Runs in `FixedPostUpdate` right after the 20 Hz simulation tick advances.
pub fn capture_sim_snapshots_fixed(
    fixed_time: Option<Res<Time<Fixed>>>,
    sim_res: Option<Res<SimWorldResource>>,
    mut prev: ResMut<PrevSimSnapshot>,
    mut curr: ResMut<CurrSimSnapshot>,
) {
    let Some(sim_res) = sim_res else {
        return;
    };
    let fixed_dt = fixed_time.map(|t| t.delta_secs()).unwrap_or(0.05);
    let world = &sim_res.0;

    // Reset both snapshots if world was reloaded / rewound to an earlier tick
    if !curr.0.initialized || world.tick < curr.0.tick {
        let snap = SimTickSnapshot::capture(world, None, fixed_dt);
        prev.0 = snap.clone();
        curr.0 = snap;
        return;
    }

    if world.tick != curr.0.tick {
        prev.0 = curr.0.clone();
        curr.0 = SimTickSnapshot::capture(world, Some(&prev.0), fixed_dt);
    } else if prev.0.tick != curr.0.tick {
        // Simulation is paused on the same tick -> settle prev onto curr
        prev.0 = curr.0.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_tick_overstep_interpolation_handles_alpha_bounds_wraparound_and_teleport_epoch() {
        let prev = EntityTransformSample {
            position: Vec3::new(10.0, 0.0, 20.0),
            facing_radians: 3.05,
            turret_facing_radians: -3.05,
            recoil_timer: 0.2,
            current_speed: 4.0,
            teleport_epoch: 1,
            rotor_angle_rad: 0.0,
        };
        let curr = EntityTransformSample {
            position: Vec3::new(14.0, 1.0, 22.0),
            facing_radians: -3.05, // crosses +PI / -PI boundary (~0.183 rad across PI)
            turret_facing_radians: 3.05,
            recoil_timer: 0.0,
            current_speed: 6.0,
            teleport_epoch: 1,
            rotor_angle_rad: 1.0,
        };

        let p0 = interpolate_entity_sample(Some(&prev), &curr, 0.0);
        assert!((p0.position - prev.position).length() < 1e-5);
        assert!(!p0.snapped_teleport);

        let p_mid = interpolate_entity_sample(Some(&prev), &curr, 0.5);
        assert!((p_mid.position - Vec3::new(12.0, 0.5, 21.0)).length() < 1e-5);
        // Shortest arc between +3.05 and -3.05 passes through +/-PI, NEVER 0.0!
        assert!(
            p_mid.facing_radians.abs() > 3.0,
            "Midpoint angle across -PI/+PI seam must stay near +/-PI, got {}",
            p_mid.facing_radians
        );
        assert!(
            p_mid.turret_facing_radians.abs() > 3.0,
            "Turret angle across -PI/+PI seam must take shortest arc, got {}",
            p_mid.turret_facing_radians
        );

        let p1 = interpolate_entity_sample(Some(&prev), &curr, 1.0);
        assert!((p1.position - curr.position).length() < 1e-5);

        // Chronoshift / Teleport epoch increment must snap immediately even at alpha = 0.25
        let mut teleported = curr;
        teleported.position = Vec3::new(96.0, 0.0, 96.0);
        teleported.teleport_epoch = 2;
        let p_snap = interpolate_entity_sample(Some(&prev), &teleported, 0.25);
        assert!(p_snap.snapped_teleport);
        assert!((p_snap.position - teleported.position).length() < 1e-5);
    }
}
