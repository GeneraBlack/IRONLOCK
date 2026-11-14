use super::destruction_events::spawn_destruction_vfx;
use super::signature_beams::{
    classify_signature_beam, signature_beam_impact_light_spec, SignatureBeamKind,
};
use super::{
    BattlefieldVfxEntity, DynamicFlashLightVfx, EnergyBeamVfx, GpuParticleInstance,
    GpuParticlePoolResources, ProjectileVfx, ShockwaveRingVfx, VfxAssets, VfxTrackerState,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{CachedFogVisibility, SocketRegistry};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{DeliveryKind, ProjectileStyle};
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_combat_and_destruction_events(
    commands: &mut Commands,
    loaded: &LoadedMod,
    world: &SimWorld,
    cached_fow: &CachedFogVisibility,
    pov_pid: u8,
    assets: &mut VfxAssets,
    mut gpu_pool: Option<&mut GpuParticlePoolResources>,
    mut socket_reg: Option<&mut SocketRegistry>,
    materials: &mut Assets<StandardMaterial>,
    interaction: &mut ClientInteractionState,
    tracker: &mut VfxTrackerState,
    camera_q: &mut Query<&mut RtsCameraRig>,
) {
    let mut primary_beam_targets = BTreeSet::new();

    for (idx, ev) in world.combat_events.iter().enumerate() {
        if let Some(target_ent) = world.entities.get(&ev.target_id) {
            if target_ent.player_id == pov_pid && tracker.under_attack_cooldown <= 0.0 {
                tracker.under_attack_cooldown = 12.0;
                let msg = if matches!(target_ent.kind, EntityKind::Building { .. }) {
                    "EVA WARNING: Our base is under attack!"
                } else {
                    "EVA: Unit under attack!"
                };
                interaction.push_eva_alert(msg);
            }
        }

        let att_ally = world.entities.get(&ev.attacker_id).is_some_and(|e| {
            e.player_id == pov_pid || (e.player_id != 0 && !world.are_hostile(pov_pid, e.player_id))
        });
        let tgt_ally = world.entities.get(&ev.target_id).is_some_and(|e| {
            e.player_id == pov_pid || (e.player_id != 0 && !world.are_hostile(pov_pid, e.player_id))
        });
        if !att_ally
            && !tgt_ally
            && !cached_fow.is_world_pos_visible(world, ev.attacker_pos)
            && !cached_fow.is_world_pos_visible(world, ev.target_pos)
        {
            continue;
        }

        let wid_low = ev.weapon_id.to_ascii_lowercase();
        let beam_kind_opt = classify_signature_beam(&ev.weapon_id, ev.projectile_style);
        let is_tesla = beam_kind_opt == Some(SignatureBeamKind::Tesla);
        let is_refracted_child = beam_kind_opt == Some(SignatureBeamKind::Prism)
            && primary_beam_targets.contains(&ev.attacker_id);
        if beam_kind_opt.is_some() {
            primary_beam_targets.insert(ev.target_id);
        }

        let resolved_muzzle = if is_refracted_child {
            None
        } else {
            socket_reg
                .as_deref_mut()
                .and_then(|reg| reg.next_muzzle_pos(ev.attacker_id))
        };
        let has_exact_muzzle = resolved_muzzle.is_some();
        let start = resolved_muzzle.unwrap_or_else(|| {
            Vec3::new(
                ev.attacker_pos[0],
                ev.attacker_pos[1] + if is_refracted_child { 0.95 } else { 1.15 },
                ev.attacker_pos[2],
            )
        });
        let end = Vec3::new(ev.target_pos[0], ev.target_pos[1] + 0.95, ev.target_pos[2]);
        let ground_y = world
            .grid
            .sample_world_height(ev.target_pos[0], ev.target_pos[2]);

        let is_heavy = ev.projectile_style == ProjectileStyle::Missile
            || wid_low.contains("cannon")
            || wid_low.contains("apocalypse")
            || ev.damage >= 65.0;

        if (beam_kind_opt.is_some() || is_heavy) && idx == 0 {
            if let Ok(mut rig) = camera_q.get_single_mut() {
                rig.add_trauma(end, if is_tesla { 0.18 } else { 0.10 });
            }
        }

        if let Some(beam_kind) = beam_kind_opt {
            let beam_origin = if has_exact_muzzle || is_refracted_child {
                start
            } else if is_tesla {
                start + Vec3::Y * 1.1
            } else {
                start + Vec3::Y * 0.45
            };
            let seed = world.tick.wrapping_add(idx as u64 * 97);
            let splinter_count = match beam_kind {
                SignatureBeamKind::Prism => {
                    if is_refracted_child {
                        2
                    } else {
                        2 + (seed % 3) as u8
                    }
                }
                SignatureBeamKind::Tesla => {
                    if ev.damage >= 95.0 {
                        3
                    } else {
                        2
                    }
                }
                _ => 0,
            };
            let lifetime = match beam_kind {
                SignatureBeamKind::RadBeam => 0.40,
                SignatureBeamKind::PsychicWave => 0.46,
                _ => 0.34,
            };

            commands.spawn((
                BattlefieldVfxEntity,
                EnergyBeamVfx {
                    start: beam_origin,
                    end,
                    is_tesla,
                    style: ev.projectile_style,
                    beam_kind,
                    splinter_count,
                    age: 0.0,
                    lifetime,
                    seed,
                },
            ));

            if !is_refracted_child
                && matches!(
                    beam_kind,
                    SignatureBeamKind::Prism | SignatureBeamKind::Tesla
                )
            {
                let (m_col, m_int, m_rng) = if is_tesla {
                    (Color::srgb(0.24, 0.74, 1.0), 118_000.0, 12.5)
                } else {
                    (Color::srgb(0.36, 0.95, 1.0), 86_000.0, 9.5)
                };
                commands.spawn((
                    BattlefieldVfxEntity,
                    DynamicFlashLightVfx {
                        age: 0.0,
                        lifetime: 0.20,
                        initial_intensity: m_int,
                    },
                    PointLight {
                        color: m_col,
                        intensity: m_int,
                        range: m_rng,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(beam_origin),
                ));
                if let Some(pool) = gpu_pool.as_deref_mut() {
                    let count = if is_tesla { 7 } else { 5 };
                    for m in 0..count {
                        let a = (m as f32 / count as f32) * std::f32::consts::TAU;
                        pool.burst_buffer.emit(GpuParticleInstance {
                            position: beam_origin,
                            velocity: Vec3::new(a.cos() * 2.4, 1.8 + (m % 2) as f32, a.sin() * 2.4),
                            age: 0.0,
                            lifetime: 0.24,
                            gravity: if is_tesla { 9.0 } else { -1.2 },
                            start_size: 0.22,
                            end_size: 0.05,
                            color_rgba: if is_tesla {
                                [0.30, 0.80, 1.0, 0.96]
                            } else {
                                [0.75, 0.98, 1.0, 0.96]
                            },
                        });
                    }
                }
            }

            let (flash_col, flash_int, flash_rng, flash_life) =
                signature_beam_impact_light_spec(beam_kind);
            commands.spawn((
                BattlefieldVfxEntity,
                DynamicFlashLightVfx {
                    age: 0.0,
                    lifetime: flash_life,
                    initial_intensity: flash_int,
                },
                PointLight {
                    color: flash_col,
                    intensity: flash_int,
                    range: flash_rng,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(end + Vec3::Y * 0.65),
            ));

            let ground_impact = Vec3::new(end.x, ground_y.max(0.0) + 0.08, end.z);
            match beam_kind {
                SignatureBeamKind::RadBeam => {
                    commands.spawn((
                        BattlefieldVfxEntity,
                        ShockwaveRingVfx {
                            center: ground_impact,
                            max_radius: 2.85,
                            age: 0.0,
                            lifetime: 1.75,
                            color: Color::srgb(0.18, 1.0, 0.14),
                        },
                    ));
                    commands.spawn((
                        BattlefieldVfxEntity,
                        ShockwaveRingVfx {
                            center: ground_impact + Vec3::Y * 0.03,
                            max_radius: 1.55,
                            age: 0.0,
                            lifetime: 1.25,
                            color: Color::srgb(0.42, 1.0, 0.28),
                        },
                    ));
                }
                SignatureBeamKind::PsychicWave => {
                    commands.spawn((
                        BattlefieldVfxEntity,
                        ShockwaveRingVfx {
                            center: ground_impact + Vec3::Y * 0.14,
                            max_radius: 2.40,
                            age: 0.0,
                            lifetime: 0.72,
                            color: Color::srgb(0.92, 0.22, 1.0),
                        },
                    ));
                    commands.spawn((
                        BattlefieldVfxEntity,
                        ShockwaveRingVfx {
                            center: end,
                            max_radius: 1.50,
                            age: 0.0,
                            lifetime: 0.92,
                            color: Color::srgb(0.98, 0.55, 1.0),
                        },
                    ));
                }
                SignatureBeamKind::Tesla => {
                    commands.spawn((
                        BattlefieldVfxEntity,
                        ShockwaveRingVfx {
                            center: ground_impact,
                            max_radius: 2.15,
                            age: 0.0,
                            lifetime: 0.36,
                            color: Color::srgb(0.28, 0.78, 1.0),
                        },
                    ));
                }
                SignatureBeamKind::Prism | SignatureBeamKind::SonicWave => {}
            }

            if let Some(pool) = gpu_pool.as_deref_mut() {
                emit_signature_beam_impact_particles(
                    &mut pool.burst_buffer,
                    beam_kind,
                    end,
                    ground_impact,
                    idx,
                );
            }
        } else {
            spawn_ballistic_tracer_and_sparks(
                commands,
                loaded,
                world,
                assets,
                gpu_pool.as_deref_mut(),
                ev,
                idx,
                start,
                end,
                has_exact_muzzle,
                is_heavy,
            );
        }
    }

    spawn_destruction_vfx(
        commands,
        world,
        cached_fow,
        pov_pid,
        assets,
        gpu_pool,
        materials,
        interaction,
        camera_q,
    );
}

fn emit_signature_beam_impact_particles(
    burst_buffer: &mut super::GpuParticleBurstBuffer,
    beam_kind: SignatureBeamKind,
    end: Vec3,
    ground_impact: Vec3,
    idx: usize,
) {
    let count = match beam_kind {
        SignatureBeamKind::Tesla => 14,
        SignatureBeamKind::Prism => 11,
        SignatureBeamKind::RadBeam => 10,
        SignatureBeamKind::PsychicWave => 9,
        SignatureBeamKind::SonicWave => 8,
    };
    for s in 0..count {
        let angle = (s as f32 / count as f32) * std::f32::consts::TAU + (idx as f32 * 0.7);
        let (origin, vel, gravity, life, col) = match beam_kind {
            SignatureBeamKind::Tesla => {
                let spd = 4.2 + (s % 4) as f32 * 2.1;
                let col = if s & 1 == 0 {
                    [0.26, 0.78, 1.0, 0.98]
                } else {
                    [0.94, 0.99, 1.0, 0.98]
                };
                (
                    ground_impact + Vec3::Y * 0.25,
                    Vec3::new(
                        angle.cos() * spd,
                        4.6 + (s % 3) as f32 * 1.9,
                        angle.sin() * spd,
                    ),
                    17.5,
                    0.44,
                    col,
                )
            }
            SignatureBeamKind::Prism => {
                let spd = 3.8 + (s % 3) as f32 * 2.0;
                let col = if s & 1 == 0 {
                    [0.28, 0.94, 1.0, 0.98]
                } else {
                    [0.98, 1.0, 1.0, 0.98]
                };
                (
                    end,
                    Vec3::new(
                        angle.cos() * spd,
                        3.6 + (s % 2) as f32 * 1.8,
                        angle.sin() * spd,
                    ),
                    13.5,
                    0.38,
                    col,
                )
            }
            SignatureBeamKind::RadBeam => (
                ground_impact + Vec3::new(angle.cos() * 0.65, 0.15, angle.sin() * 0.65),
                Vec3::new(
                    angle.cos() * 1.6,
                    1.9 + (s % 3) as f32 * 0.6,
                    angle.sin() * 1.6,
                ),
                -0.65,
                0.88,
                [0.18, 1.0, 0.14, 0.88],
            ),
            SignatureBeamKind::PsychicWave => (
                end + Vec3::new(angle.cos() * 0.55, -0.15, angle.sin() * 0.55),
                Vec3::new(
                    -angle.sin() * 1.8,
                    1.6 + (s % 2) as f32 * 0.7,
                    angle.cos() * 1.8,
                ),
                -0.85,
                0.76,
                [0.92, 0.22, 1.0, 0.92],
            ),
            SignatureBeamKind::SonicWave => (
                end,
                Vec3::new(angle.cos() * 3.4, 2.2, angle.sin() * 3.4),
                8.0,
                0.36,
                [0.58, 0.94, 1.0, 0.92],
            ),
        };
        burst_buffer.emit(GpuParticleInstance {
            position: origin,
            velocity: vel,
            age: 0.0,
            lifetime: life,
            gravity,
            start_size: 0.24,
            end_size: if gravity < 0.0 { 0.48 } else { 0.06 },
            color_rgba: col,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_ballistic_tracer_and_sparks(
    commands: &mut Commands,
    loaded: &LoadedMod,
    world: &SimWorld,
    assets: &VfxAssets,
    gpu_pool: Option<&mut GpuParticlePoolResources>,
    ev: &rts_simulation::combat::CombatEvent,
    idx: usize,
    start: Vec3,
    end: Vec3,
    has_exact_muzzle: bool,
    is_heavy: bool,
) {
    let dir = (end - start).normalize_or_zero();
    let rot = if dir.length_squared() > 1e-4 {
        Quat::from_rotation_arc(Vec3::Z, dir)
    } else {
        Quat::IDENTITY
    };
    let uses_sim_projectile = loaded
        .weapons
        .get(&ev.weapon_id)
        .is_some_and(|w| matches!(w.delivery, DeliveryKind::Ballistic | DeliveryKind::Missile))
        || world
            .projectiles
            .iter()
            .any(|p| p.attacker_id == ev.attacker_id && p.weapon_id == ev.weapon_id);

    if !uses_sim_projectile {
        let scale = if is_heavy {
            Vec3::new(1.1, 1.1, 1.35)
        } else {
            Vec3::new(0.58, 0.58, 0.90)
        };
        commands.spawn((
            BattlefieldVfxEntity,
            ProjectileVfx {
                start,
                end,
                progress: 0.0,
                speed: if is_heavy { 36.0 } else { 56.0 },
                arc_height: 0.12,
            },
            Mesh3d(assets.shell_mesh.clone()),
            MeshMaterial3d(assets.shell_mat.clone()),
            Transform::from_translation(start)
                .with_rotation(rot)
                .with_scale(scale),
        ));
    }
    if is_heavy {
        let flash_pos = start + dir * if has_exact_muzzle { 0.25 } else { 0.9 };
        commands.spawn((
            BattlefieldVfxEntity,
            DynamicFlashLightVfx {
                age: 0.0,
                lifetime: 0.14,
                initial_intensity: 65_000.0,
            },
            PointLight {
                color: Color::srgb(1.0, 0.72, 0.25),
                intensity: 65_000.0,
                range: 10.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(flash_pos),
        ));
    }
    let spark_count = if ev.damage >= 50.0 { 9 } else { 4 };
    if let Some(pool) = gpu_pool {
        for s in 0..spark_count {
            let angle =
                (s as f32 / spark_count as f32) * std::f32::consts::TAU + (idx as f32 * 0.7);
            let speed = 3.5 + (s % 3) as f32 * 1.8;
            pool.burst_buffer.emit(GpuParticleInstance {
                position: end,
                velocity: Vec3::new(
                    angle.cos() * speed,
                    3.8 + (s % 2) as f32 * 2.0,
                    angle.sin() * speed,
                ),
                age: 0.0,
                lifetime: 0.40,
                gravity: 16.0,
                start_size: 0.24,
                end_size: 0.06,
                color_rgba: [1.0, 0.56, 0.14, 0.96],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::FixedEntitySockets;
    use crate::vfx::{
        setup_vfx_assets, spawn_combat_and_destruction_vfx, update_and_draw_vfx,
        VfxDynamicBatchEntity,
    };
    use crate::{ModResource, SimWorldResource};
    use rts_simulation::combat::CombatEvent;
    use std::path::PathBuf;

    #[test]
    fn combat_event_pipeline_spawns_refracted_prism_multi_strand_tesla_rad_and_psychic_vfx() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2 mod path");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        world.tick = 1;
        world.combat_events = vec![
            CombatEvent {
                attacker_id: 9001,
                target_id: 9002,
                attacker_pos: [10.0, 0.0, 10.0],
                target_pos: [18.0, 0.0, 10.0],
                weapon_id: "prism_cannon".to_string(),
                damage: 85.0,
                projectile_style: ProjectileStyle::Laser,
            },
            CombatEvent {
                attacker_id: 9002,
                target_id: 9003,
                attacker_pos: [18.0, 0.0, 10.0],
                target_pos: [22.0, 0.0, 14.0],
                weapon_id: "prism_cannon".to_string(),
                damage: 42.5,
                projectile_style: ProjectileStyle::Laser,
            },
            CombatEvent {
                attacker_id: 9004,
                target_id: 9005,
                attacker_pos: [30.0, 0.0, 12.0],
                target_pos: [36.0, 0.0, 16.0],
                weapon_id: "tesla_bolt".to_string(),
                damage: 120.0,
                projectile_style: ProjectileStyle::Tesla,
            },
            CombatEvent {
                attacker_id: 9006,
                target_id: 9007,
                attacker_pos: [40.0, 0.0, 20.0],
                target_pos: [45.0, 0.0, 24.0],
                weapon_id: "rad_beam_weapon".to_string(),
                damage: 60.0,
                projectile_style: ProjectileStyle::Radiation,
            },
            CombatEvent {
                attacker_id: 9008,
                target_id: 9009,
                attacker_pos: [50.0, 0.0, 20.0],
                target_pos: [54.0, 0.0, 25.0],
                weapon_id: "psychic_jab".to_string(),
                damage: 35.0,
                projectile_style: ProjectileStyle::MindControl,
            },
        ];

        let mut socket_reg = SocketRegistry::default();
        let mut victim_sockets = FixedEntitySockets::default();
        victim_sockets.upsert_named("Muzzle_01", Vec3::new(99.0, 9.0, 99.0));
        victim_sockets.muzzles[0] = Vec3::new(99.0, 9.0, 99.0);
        victim_sockets.muzzle_len = 1;
        socket_reg.entries.insert(9002, victim_sockets);

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<crate::vfx::GpuParticleShaderMaterial>()
            .insert_resource(CachedFogVisibility::default())
            .init_resource::<ClientInteractionState>()
            .init_resource::<VfxTrackerState>()
            .insert_resource(socket_reg)
            .insert_resource(ModResource(loaded))
            .insert_resource(SimWorldResource(world))
            .add_systems(Startup, setup_vfx_assets)
            .add_systems(
                Update,
                (spawn_combat_and_destruction_vfx, update_and_draw_vfx).chain(),
            );

        app.update();
        app.update();

        let ecs = app.world_mut();
        let beams: Vec<(SignatureBeamKind, Vec3, Vec3, u8)> = ecs
            .query::<&EnergyBeamVfx>()
            .iter(ecs)
            .map(|b| (b.beam_kind, b.start, b.end, b.splinter_count))
            .collect();
        assert_eq!(beams.len(), 5);

        let refracted = beams
            .iter()
            .find(|(k, _, end, _)| *k == SignatureBeamKind::Prism && (end.x - 22.0).abs() < 0.01)
            .expect("refracted child prism beam");
        assert!(
            (refracted.1 - Vec3::new(18.0, 0.95, 10.0)).length() < 0.05,
            "Refracted Prism splinter must chain from victim hit point, not victim Muzzle_01"
        );

        let tesla = beams
            .iter()
            .find(|(k, _, _, _)| *k == SignatureBeamKind::Tesla)
            .expect("tesla beam");
        assert_eq!(tesla.3, 3);

        let ring_lifetimes: Vec<f32> = ecs
            .query::<&ShockwaveRingVfx>()
            .iter(ecs)
            .map(|r| r.lifetime)
            .collect();
        assert!(
            ring_lifetimes.iter().any(|l| (*l - 1.75).abs() < 0.01),
            "Rad-beam must spawn lingering ground radiation ring glow"
        );
        assert!(
            ring_lifetimes.iter().any(|l| (*l - 0.92).abs() < 0.01),
            "Psychic wave must spawn concentric psionic target ring"
        );
        assert!(ecs.query::<&DynamicFlashLightVfx>().iter(ecs).count() >= 7);
        assert_eq!(ecs.query::<&VfxDynamicBatchEntity>().iter(ecs).count(), 1);
    }
}
