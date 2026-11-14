use super::{GpuParticleBurstBuffer, GpuParticleInstance};
use crate::rendering::{CachedFogVisibility, SocketRegistry};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::UnitRole;
use rts_simulation::world::{EntityKind, HarvesterPhase, SimWorld, TileVisibility};

/// Authentic Red Alert 2 health-ratio damage smoke & fire tier (`Phase V6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DamageSmokeTier {
    /// Green health (`health >= 50%` or dead): zero damage smoke or flames.
    #[default]
    Healthy,
    /// Yellow damage state (`25% <= health < 50%`): crisp, localized rising dark-gray smoke puffs.
    YellowSmoke,
    /// Red critical state (`0% < health < 25%`): dense dark-gray smoke + flickering orange-gold fire tongues.
    RedCriticalFire,
}

/// Classifies an entity's health into RA2's exact `< 50%` yellow smoke and `< 25%` red critical fire states.
#[inline]
pub fn classify_damage_smoke_tier(health: f32, max_health: f32) -> DamageSmokeTier {
    if !health.is_finite() || health <= 0.0 {
        return DamageSmokeTier::Healthy;
    }
    let ratio = (health / max_health.max(1.0)).clamp(0.0, 1.0);
    if ratio < 0.25 {
        DamageSmokeTier::RedCriticalFire
    } else if ratio < 0.50 {
        DamageSmokeTier::YellowSmoke
    } else {
        DamageSmokeTier::Healthy
    }
}

/// Determines whether a `Unit` entity is a mechanical vehicle/aircraft/ship/harvester that should
/// emit localized engine/hull damage smoke & fire (excluding biological infantry and marine animals).
pub fn is_mechanical_smoke_unit(unit_id: &str, max_health: f32, loaded: &LoadedMod) -> bool {
    let low = unit_id.trim().to_ascii_lowercase();
    if low.contains("dolphin") || low.contains("squid") || low.contains("dog") {
        return false;
    }
    if let Some(udef) = loaded.units.get(unit_id) {
        matches!(udef.role, UnitRole::Vehicle | UnitRole::Harvester)
    } else {
        max_health >= 180.0
    }
}

/// Resolves localized primary (`DamageSmoke_01`) and optional secondary (`DamageFire` / `DamageSmoke_02`)
/// world-space damage sockets from `SocketRegistry`, falling back cleanly to roof/engine-deck offsets.
pub fn resolve_damage_smoke_sockets(
    sim_id: u64,
    is_building: bool,
    position: [f32; 3],
    facing_radians: f32,
    ground_y: f32,
    socket_reg: Option<&SocketRegistry>,
) -> (Vec3, Option<Vec3>) {
    if let Some(reg) = socket_reg {
        let primary = reg
            .get_socket(sim_id, "DamageSmoke_01")
            .or_else(|| reg.damage_smoke_pos(sim_id));
        let secondary = reg
            .get_socket(sim_id, "DamageFire")
            .or_else(|| reg.get_socket(sim_id, "DamageSmoke_02"))
            .or_else(|| (!is_building).then(|| reg.exhaust_pos(sim_id)).flatten());
        if let Some(p) = primary {
            let distinct_sec = secondary.filter(|s| s.distance_squared(p) > 0.04);
            return (p, distinct_sec);
        }
    }

    let base_y = position[1].max(ground_y);
    if is_building {
        let primary = Vec3::new(position[0] + 0.22, base_y + 1.75, position[2] - 0.16);
        let secondary = Some(Vec3::new(
            position[0] - 0.34,
            base_y + 1.42,
            position[2] + 0.28,
        ));
        (primary, secondary)
    } else {
        let (sf, cf) = facing_radians.sin_cos();
        // Rear engine-deck fallback keeps turret and front team-colored armor crisp & clean
        let primary = Vec3::new(
            position[0] - sf * 0.52,
            base_y + 0.96,
            position[2] - cf * 0.52,
        );
        (primary, None)
    }
}

/// Resolves localized `Exhaust_01` socket world position for moving ground vehicles.
pub fn resolve_vehicle_exhaust_socket(
    sim_id: u64,
    position: [f32; 3],
    facing_radians: f32,
    ground_y: f32,
    socket_reg: Option<&SocketRegistry>,
) -> Vec3 {
    if let Some(reg) = socket_reg {
        if let Some(p) = reg
            .get_socket(sim_id, "Exhaust_01")
            .or_else(|| reg.exhaust_pos(sim_id))
        {
            return p;
        }
    }
    let (sf, cf) = facing_radians.sin_cos();
    Vec3::new(
        position[0] - sf * 1.05,
        position[1].max(ground_y) + 0.34,
        position[2] - cf * 1.05,
    )
}

/// Emits localized rising dark-gray smoke puffs (`YellowSmoke`, `< 50%`) and flickering orange-gold
/// fire tongues (`RedCriticalFire`, `< 25%`) into `burst_buffer`, returning emitted particle count.
pub fn emit_localized_damage_plume(
    burst_buffer: &mut GpuParticleBurstBuffer,
    primary_anchor: Vec3,
    secondary_anchor: Option<Vec3>,
    tier: DamageSmokeTier,
    is_building: bool,
    seed: u64,
) -> usize {
    if tier == DamageSmokeTier::Healthy {
        return 0;
    }

    let jx = (((seed & 0xff) as f32 / 255.0) - 0.5) * if is_building { 0.24 } else { 0.14 };
    let jz = ((((seed >> 8) & 0xff) as f32 / 255.0) - 0.5) * if is_building { 0.24 } else { 0.14 };
    let smoke_origin = primary_anchor + Vec3::new(jx, 0.0, jz);
    let is_critical = tier == DamageSmokeTier::RedCriticalFire;

    // 1. Crisp, localized rising dark-gray smoke puff anchored tightly on `DamageSmoke_01`
    let (smoke_start, smoke_end, smoke_vy, smoke_life) = if is_building {
        if is_critical {
            (0.34, 0.76, 2.45, 1.28)
        } else {
            (0.28, 0.62, 2.05, 1.12)
        }
    } else if is_critical {
        (0.25, 0.56, 2.10, 0.98)
    } else {
        (0.20, 0.46, 1.75, 0.86)
    };
    let smoke_alpha = if is_critical { 0.78 } else { 0.66 };
    burst_buffer.emit(GpuParticleInstance {
        position: smoke_origin,
        velocity: Vec3::new(jx * 0.45, smoke_vy, jz * 0.45),
        age: 0.0,
        lifetime: smoke_life,
        gravity: -0.45,
        start_size: smoke_start,
        end_size: smoke_end,
        color_rgba: [0.17, 0.18, 0.20, smoke_alpha],
    });
    let mut emitted = 1usize;

    // 2. Red critical state (`< 25%`): localized flickering orange-gold fire tongues at socket base
    if is_critical {
        let flame_flicker = (((seed >> 16) & 0xff) as f32) / 255.0;
        let flame_base = primary_anchor + Vec3::new(jx * 0.45, -0.05, jz * 0.45);
        burst_buffer.emit(GpuParticleInstance {
            position: flame_base,
            velocity: Vec3::new(jx * 0.9, 2.75 + flame_flicker * 0.85, jz * 0.9),
            age: 0.0,
            lifetime: 0.34 + flame_flicker * 0.10,
            gravity: 3.2,
            start_size: if is_building { 0.30 } else { 0.22 },
            end_size: 0.07,
            color_rgba: [1.0, 0.46, 0.10, 0.94],
        });
        burst_buffer.emit(GpuParticleInstance {
            position: flame_base + Vec3::Y * 0.04,
            velocity: Vec3::new(-jz * 0.6, 3.15 + flame_flicker * 0.65, jx * 0.6),
            age: 0.0,
            lifetime: 0.24 + flame_flicker * 0.08,
            gravity: 2.6,
            start_size: if is_building { 0.20 } else { 0.15 },
            end_size: 0.04,
            color_rgba: [1.0, 0.84, 0.24, 0.96],
        });
        emitted += 2;

        if let Some(sec) = secondary_anchor {
            if is_building && (seed & 1 == 0) {
                burst_buffer.emit(GpuParticleInstance {
                    position: sec,
                    velocity: Vec3::new(-jx * 0.6, 2.55, -jz * 0.6),
                    age: 0.0,
                    lifetime: 0.36,
                    gravity: 3.0,
                    start_size: 0.26,
                    end_size: 0.06,
                    color_rgba: [1.0, 0.56, 0.14, 0.92],
                });
                emitted += 1;
            }
        }
    }

    emitted
}

/// Emits periodic GPU-batched construction sparks, socket-bound `< 50%` / `< 25%` damage smoke & fire,
/// `Exhaust_01` moving vehicle dust/exhaust, burrowed dirt, harvester ore sparks, and wreck smoke.
pub(super) fn emit_battlefield_ambient_and_damage_vfx(
    world: &SimWorld,
    loaded: &LoadedMod,
    cached_fow: &CachedFogVisibility,
    socket_reg: Option<&SocketRegistry>,
    burst_buffer: &mut GpuParticleBurstBuffer,
) {
    for ent in world.entities.values() {
        if ent.health <= 0.0 || !cached_fow.is_entity_visible(world, loaded, ent) {
            continue;
        }
        let ground_y = world
            .grid
            .sample_world_height(ent.position[0], ent.position[2]);
        let hash = ent.id.wrapping_mul(2_654_435_761) ^ world.tick.wrapping_mul(40_503);

        match &ent.kind {
            EntityKind::Building { .. } => {
                if ent.construction_progress < 0.99 {
                    let angle = ((hash & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                    let active_y = ground_y + 0.20 + ent.construction_progress * 2.15;
                    burst_buffer.emit(GpuParticleInstance {
                        position: Vec3::new(
                            ent.position[0] + angle.cos() * 1.15,
                            active_y,
                            ent.position[2] + angle.sin() * 1.15,
                        ),
                        velocity: Vec3::new(angle.cos() * 2.8, 3.2, angle.sin() * 2.8),
                        age: 0.0,
                        lifetime: 0.34,
                        gravity: 11.0,
                        start_size: 0.24,
                        end_size: 0.06,
                        color_rgba: if hash & 1 == 0 {
                            [0.35, 0.95, 1.0, 0.95]
                        } else {
                            [1.0, 0.85, 0.32, 0.95]
                        },
                    });
                    burst_buffer.emit(GpuParticleInstance {
                        position: Vec3::new(
                            ent.position[0] - angle.cos() * 1.4,
                            ground_y + 0.18,
                            ent.position[2] - angle.sin() * 1.4,
                        ),
                        velocity: Vec3::new(angle.cos() * 0.9, 0.7, angle.sin() * 0.9),
                        age: 0.0,
                        lifetime: 0.58,
                        gravity: -0.25,
                        start_size: 0.36,
                        end_size: 0.82,
                        color_rgba: [0.52, 0.48, 0.42, 0.52],
                    });
                } else {
                    let tier = classify_damage_smoke_tier(ent.health, ent.max_health);
                    if tier != DamageSmokeTier::Healthy {
                        let (primary, secondary) = resolve_damage_smoke_sockets(
                            ent.id,
                            true,
                            ent.position,
                            ent.facing_radians,
                            ground_y,
                            socket_reg,
                        );
                        emit_localized_damage_plume(
                            burst_buffer,
                            primary,
                            secondary,
                            tier,
                            true,
                            hash,
                        );
                    }
                }

                if ent.upgrading_to.is_some() {
                    let up_hash = ent.id.wrapping_mul(2_246_822_519) ^ world.tick;
                    let angle = ((up_hash & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                    burst_buffer.emit(GpuParticleInstance {
                        position: Vec3::new(
                            ent.position[0] + angle.cos() * 1.4,
                            ent.position[1].max(ground_y) + 1.1,
                            ent.position[2] + angle.sin() * 1.4,
                        ),
                        velocity: Vec3::new(angle.cos() * 1.8, 3.6, angle.sin() * 1.8),
                        age: 0.0,
                        lifetime: 0.38,
                        gravity: 6.0,
                        start_size: 0.26,
                        end_size: 0.08,
                        color_rgba: [1.0, 0.88, 0.28, 0.96],
                    });
                }
            }
            EntityKind::Unit { unit_id } => {
                let is_mech = is_mechanical_smoke_unit(unit_id, ent.max_health, loaded);
                if is_mech && !ent.path_waypoints.is_empty() && !ent.burrowed {
                    let exhaust_origin = resolve_vehicle_exhaust_socket(
                        ent.id,
                        ent.position,
                        ent.facing_radians,
                        ground_y,
                        socket_reg,
                    );
                    burst_buffer.emit(GpuParticleInstance {
                        position: exhaust_origin,
                        velocity: Vec3::new(0.0, 0.82, 0.0),
                        age: 0.0,
                        lifetime: 0.50,
                        gravity: -0.22,
                        start_size: 0.24,
                        end_size: 0.54,
                        color_rgba: [0.44, 0.41, 0.37, 0.44],
                    });
                }

                if is_mech && !ent.burrowed {
                    let tier = classify_damage_smoke_tier(ent.health, ent.max_health);
                    if tier != DamageSmokeTier::Healthy {
                        let (primary, secondary) = resolve_damage_smoke_sockets(
                            ent.id,
                            false,
                            ent.position,
                            ent.facing_radians,
                            ground_y,
                            socket_reg,
                        );
                        emit_localized_damage_plume(
                            burst_buffer,
                            primary,
                            secondary,
                            tier,
                            false,
                            hash,
                        );
                    }
                }

                if ent.burrowed {
                    burst_buffer.emit(GpuParticleInstance {
                        position: Vec3::new(ent.position[0], ground_y + 0.16, ent.position[2]),
                        velocity: Vec3::new(0.0, 2.2, 0.0),
                        age: 0.0,
                        lifetime: 0.45,
                        gravity: 6.5,
                        start_size: 0.38,
                        end_size: 0.56,
                        color_rgba: [0.36, 0.28, 0.18, 0.78],
                    });
                }

                if let Some(harv) = &ent.harvester_state {
                    if matches!(harv.phase, HarvesterPhase::Harvesting(_)) {
                        let front_x = ent.position[0] + ent.facing_radians.sin() * 1.25;
                        let front_z = ent.position[2] + ent.facing_radians.cos() * 1.25;
                        burst_buffer.emit(GpuParticleInstance {
                            position: Vec3::new(
                                front_x,
                                world.grid.sample_world_height(front_x, front_z) + 0.45,
                                front_z,
                            ),
                            velocity: Vec3::new(0.0, 2.8, 0.0),
                            age: 0.0,
                            lifetime: 0.35,
                            gravity: 9.0,
                            start_size: 0.24,
                            end_size: 0.06,
                            color_rgba: [1.0, 0.88, 0.24, 0.95],
                        });
                    }
                }
            }
        }
    }

    for scar in &world.battlefield_scars {
        if scar.is_wreck
            && scar.remaining_secs > 5.0
            && cached_fow.world_pos_visibility(world, scar.position) != TileVisibility::Unexplored
        {
            let ground_y = world
                .grid
                .sample_world_height(scar.position[0], scar.position[2]);
            burst_buffer.emit(GpuParticleInstance {
                position: Vec3::new(scar.position[0], ground_y + 0.52, scar.position[2]),
                velocity: Vec3::new(0.14, 1.70, -0.10),
                age: 0.0,
                lifetime: 0.92,
                gravity: -0.50,
                start_size: 0.32,
                end_size: 0.78,
                color_rgba: [0.16, 0.16, 0.18, 0.64],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::FixedEntitySockets;

    #[test]
    fn damage_smoke_tier_enforces_exact_ra2_50_percent_yellow_and_25_percent_red_thresholds() {
        assert_eq!(
            classify_damage_smoke_tier(400.0, 400.0),
            DamageSmokeTier::Healthy
        );
        assert_eq!(
            classify_damage_smoke_tier(200.0, 400.0),
            DamageSmokeTier::Healthy
        );
        assert_eq!(
            classify_damage_smoke_tier(199.0, 400.0),
            DamageSmokeTier::YellowSmoke
        );
        assert_eq!(
            classify_damage_smoke_tier(100.0, 400.0),
            DamageSmokeTier::YellowSmoke
        );
        assert_eq!(
            classify_damage_smoke_tier(99.0, 400.0),
            DamageSmokeTier::RedCriticalFire
        );
        assert_eq!(
            classify_damage_smoke_tier(1.0, 400.0),
            DamageSmokeTier::RedCriticalFire
        );
        assert_eq!(
            classify_damage_smoke_tier(0.0, 400.0),
            DamageSmokeTier::Healthy
        );
    }

    #[test]
    fn socket_bound_damage_plume_anchors_to_damage_smoke_socket_and_emits_fire_only_below_25_pct() {
        let mut reg = SocketRegistry::default();
        let mut sockets = FixedEntitySockets::default();
        let custom_smoke = Vec3::new(14.5, 3.25, 22.0);
        let custom_exhaust = Vec3::new(13.2, 0.55, 20.8);
        sockets.damage_smoke = Some(custom_smoke);
        sockets.exhaust = Some(custom_exhaust);
        sockets.upsert_named("DamageSmoke_01", custom_smoke);
        sockets.upsert_named("Exhaust_01", custom_exhaust);
        reg.entries.insert(7, sockets);

        let (resolved_dmg, _) =
            resolve_damage_smoke_sockets(7, true, [14.0, 0.0, 22.0], 0.0, 0.0, Some(&reg));
        assert!((resolved_dmg - custom_smoke).length() < 1e-4);

        let resolved_exh =
            resolve_vehicle_exhaust_socket(7, [14.0, 0.0, 22.0], 0.0, 0.0, Some(&reg));
        assert!((resolved_exh - custom_exhaust).length() < 1e-4);

        // Fallback when SocketRegistry lacks entry uses clean roof / rear-engine offsets
        let (fallback_tank, _) =
            resolve_damage_smoke_sockets(99, false, [10.0, 0.0, 10.0], 0.0, 0.0, Some(&reg));
        assert!((fallback_tank - Vec3::new(10.0, 0.96, 9.48)).length() < 1e-3);

        let mut buf = GpuParticleBurstBuffer::default();
        assert_eq!(
            emit_localized_damage_plume(
                &mut buf,
                custom_smoke,
                None,
                DamageSmokeTier::Healthy,
                true,
                1
            ),
            0
        );

        let yellow_n = emit_localized_damage_plume(
            &mut buf,
            custom_smoke,
            None,
            DamageSmokeTier::YellowSmoke,
            false,
            123,
        );
        assert_eq!(yellow_n, 1);
        assert!(
            buf.instances[0].color_rgba[0] < 0.30,
            "Yellow state emits dark-gray smoke only"
        );

        buf.clear();
        let red_n = emit_localized_damage_plume(
            &mut buf,
            custom_smoke,
            Some(custom_smoke + Vec3::new(0.8, -0.2, 0.4)),
            DamageSmokeTier::RedCriticalFire,
            true,
            2,
        );
        assert!(red_n >= 3);
        assert!(
            buf.instances.iter().any(|p| p.color_rgba[0] > 0.95
                && p.color_rgba[1] > 0.40
                && p.color_rgba[2] < 0.30),
            "Red critical state (<25%) must emit flickering orange-gold fire tongues"
        );
    }
}
