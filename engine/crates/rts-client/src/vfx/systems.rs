use super::*;
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use bevy::prelude::*;
use rts_sdk::schema::ProjectileStyle;
use rts_simulation::world::{EntityKind, SimWorld};

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_combat_and_destruction_events(
    commands: &mut Commands,
    world: &SimWorld,
    assets: &VfxAssets,
    materials: &mut Assets<StandardMaterial>,
    interaction: &mut ClientInteractionState,
    tracker: &mut VfxTrackerState,
    camera_q: &mut Query<&mut RtsCameraRig>,
) {
    for (idx, ev) in world.combat_events.iter().enumerate() {
        let start = Vec3::new(
            ev.attacker_pos[0],
            ev.attacker_pos[1] + 1.15,
            ev.attacker_pos[2],
        );
        let end = Vec3::new(
            ev.target_pos[0],
            ev.target_pos[1] + 0.95,
            ev.target_pos[2],
        );
        let wid_low = ev.weapon_id.to_lowercase();
        let is_tesla = ev.projectile_style == ProjectileStyle::Tesla || wid_low.contains("tesla");
        let is_beam = is_tesla
            || matches!(
                ev.projectile_style,
                ProjectileStyle::Laser
                    | ProjectileStyle::Radiation
                    | ProjectileStyle::Sonic
                    | ProjectileStyle::MindControl
            )
            || wid_low.contains("prism")
            || wid_low.contains("radbeam")
            || wid_low.contains("psy");
        let is_heavy = ev.projectile_style == ProjectileStyle::Missile
            || wid_low.contains("cannon")
            || wid_low.contains("apocalypse")
            || ev.damage >= 65.0;

        if (is_beam || is_heavy) && idx == 0 {
            if let Ok(mut rig) = camera_q.get_single_mut() {
                rig.add_trauma(end, if is_tesla { 0.18 } else { 0.10 });
            }
        }

        if let Some(target_ent) = world.entities.get(&ev.target_id) {
            if target_ent.player_id == interaction.local_player_id
                && tracker.under_attack_cooldown <= 0.0
            {
                tracker.under_attack_cooldown = 12.0;
                if matches!(target_ent.kind, EntityKind::Building { .. }) {
                    interaction.push_eva_alert("EVA WARNING: Our base is under attack!");
                } else {
                    interaction.push_eva_alert("EVA: Unit under attack!");
                }
            }
        }

        if is_beam {
            commands.spawn(EnergyBeamVfx {
                start: if is_tesla {
                    start + Vec3::Y * 1.1
                } else {
                    start + Vec3::Y * 0.45
                },
                end,
                is_tesla,
                style: ev.projectile_style,
                age: 0.0,
                lifetime: 0.32,
                seed: world.tick.wrapping_add(idx as u64 * 97),
            });
            let flash_col = match ev.projectile_style {
                ProjectileStyle::Radiation => Color::srgb(0.25, 1.0, 0.20),
                ProjectileStyle::MindControl => Color::srgb(0.90, 0.25, 1.0),
                ProjectileStyle::Sonic => Color::srgb(0.55, 0.90, 1.0),
                _ => Color::srgb(0.3, 0.92, 1.0),
            };
            commands.spawn((
                DynamicFlashLightVfx {
                    age: 0.0,
                    lifetime: 0.22,
                    initial_intensity: 95_000.0,
                },
                PointLight {
                    color: flash_col,
                    intensity: 95_000.0,
                    range: 14.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(end + Vec3::Y * 0.8),
            ));
        } else {
            let dir = (end - start).normalize_or_zero();
            let rot = if dir.length_squared() > 1e-4 {
                Quat::from_rotation_arc(Vec3::Z, dir)
            } else {
                Quat::IDENTITY
            };

            // Spawn 3D ballistic shell / missile / tracer
            commands.spawn((
                ProjectileVfx {
                    start,
                    end,
                    progress: 0.0,
                    speed: if is_heavy { 36.0 } else { 52.0 },
                    arc_height: if ev.projectile_style == ProjectileStyle::Missile {
                        1.45
                    } else if is_heavy {
                        0.85
                    } else {
                        0.15
                    },
                },
                Mesh3d(assets.shell_mesh.clone()),
                MeshMaterial3d(assets.shell_mat.clone()),
                Transform::from_translation(start)
                    .with_rotation(rot)
                    .with_scale(if is_heavy {
                        Vec3::new(1.3, 1.3, 1.5)
                    } else {
                        Vec3::new(0.65, 0.65, 0.9)
                    }),
            ));

            if is_heavy {
                // Muzzle flash light
                commands.spawn((
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
                    Transform::from_translation(start + dir * 0.9),
                ));
            }
        }

        // Impact sparks at target
        let spark_count = if ev.damage >= 50.0 { 7 } else { 3 };
        let spark_mat = if is_beam {
            assets.plasma_cyan_mat.clone()
        } else {
            assets.fire_mat.clone()
        };
        for s in 0..spark_count {
            let angle = (s as f32 / spark_count as f32) * std::f32::consts::TAU
                + (idx as f32 * 0.7);
            let speed = 3.5 + (s % 3) as f32 * 1.8;
            let vel = Vec3::new(angle.cos() * speed, 3.8 + (s % 2) as f32 * 2.0, angle.sin() * speed);
            commands.spawn((
                ParticleDebrisVfx {
                    velocity: vel,
                    age: 0.0,
                    lifetime: 0.38,
                    gravity: 16.0,
                    initial_scale: 0.75,
                },
                Mesh3d(assets.spark_mesh.clone()),
                MeshMaterial3d(spark_mat.clone()),
                Transform::from_translation(end),
            ));
        }
    }

    // 3. Spawn multi-stage 3D explosions for destroyed units & buildings (or deconstruction VFX when sold)
    for dest in &world.destruction_events {
        let center = Vec3::new(dest.position[0], 0.9, dest.position[2]);
        let max_r = dest.radius;

        if dest.is_sold {
            // Building was sold -> spawn golden credit-recycle ring & upward deconstruction sparks
            commands.spawn(ShockwaveRingVfx {
                center: Vec3::new(dest.position[0], 0.22, dest.position[2]),
                max_radius: 4.0,
                age: 0.0,
                lifetime: 0.85,
                color: Color::srgb(1.0, 0.86, 0.22),
            });
            for i in 0..14 {
                let a = (i as f32 / 14.0) * std::f32::consts::TAU;
                let r = 1.2 + (i % 3) as f32 * 0.55;
                commands.spawn((
                    ParticleDebrisVfx {
                        velocity: Vec3::new(a.cos() * 1.4, 3.8 + (i % 3) as f32, a.sin() * 1.4),
                        age: 0.0,
                        lifetime: 0.72,
                        gravity: 4.5,
                        initial_scale: 0.68,
                    },
                    Mesh3d(assets.spark_mesh.clone()),
                    MeshMaterial3d(if i % 2 == 0 {
                        assets.shell_mat.clone()
                    } else {
                        assets.plasma_cyan_mat.clone()
                    }),
                    Transform::from_xyz(
                        dest.position[0] + a.cos() * r,
                        1.1,
                        dest.position[2] + a.sin() * r,
                    ),
                ));
            }
            continue;
        }

        if let Ok(mut rig) = camera_q.get_single_mut() {
            rig.add_trauma(center, if dest.is_building { 0.48 } else { 0.26 });
        }
        if dest.player_id == interaction.local_player_id {
            if dest.is_building {
                interaction.push_eva_alert("EVA: Structure lost!");
            } else {
                interaction.push_eva_alert("EVA: Unit lost.");
            }
        } else if dest.is_building {
            interaction.push_eva_alert("EVA: Enemy structure destroyed!");
        }

        // Expanding glowing fireball core
        commands.spawn((
            ExplosionCoreVfx {
                age: 0.0,
                lifetime: if dest.is_building { 0.65 } else { 0.42 },
                max_radius: max_r,
            },
            Mesh3d(assets.fireball_mesh.clone()),
            MeshMaterial3d(assets.fire_mat.clone()),
            Transform::from_translation(center).with_scale(Vec3::splat(0.3)),
        ));

        // Spawn authentic RA2 VFX Flipbook Billboard if present in LoadedMod.vfx_catalog
        let sheet_key = if dest.is_building {
            "explold"
        } else if max_r >= 2.2 {
            "explomed"
        } else {
            "explosml"
        };
        if let Some((sheet_img, f_count, cols, rows)) = assets
            .flipbook_sheets
            .get(sheet_key)
            .or_else(|| assets.flipbook_sheets.values().next())
        {
            let frames = (*f_count).max(1);
            let c_cnt = (*cols).max(1);
            let r_cnt = (*rows).max(1);
            let mat_h = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(sheet_img.clone()),
                emissive: LinearRgba::new(1.8, 1.2, 0.5, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                uv_transform: bevy::math::Affine2::from_scale_angle_translation(
                    Vec2::new(1.0 / c_cnt as f32, 1.0 / r_cnt as f32),
                    0.0,
                    Vec2::ZERO,
                ),
                ..default()
            });
            let b_scale = if dest.is_building { 4.2 } else { 2.6 };
            commands.spawn((
                VfxFlipbookBillboard {
                    age: 0.0,
                    lifetime: (frames as f32 / 18.0).clamp(0.35, 1.15),
                    frame_count: frames,
                    columns: c_cnt,
                    rows: r_cnt,
                    max_scale: b_scale,
                    mat_handle: mat_h.clone(),
                },
                Mesh3d(assets.billboard_mesh.clone()),
                MeshMaterial3d(mat_h),
                Transform::from_translation(center + Vec3::Y * 0.65)
                    .with_rotation(Quat::from_rotation_x(-0.55))
                    .with_scale(Vec3::splat(b_scale)),
            ));
        }

        // Ground shockwave ring
        commands.spawn(ShockwaveRingVfx {
            center: Vec3::new(dest.position[0], 0.16, dest.position[2]),
            max_radius: max_r * 2.4,
            age: 0.0,
            lifetime: if dest.is_building { 0.75 } else { 0.48 },
            color: Color::srgb(1.0, 0.68, 0.18),
        });

        // Persistent ground scorch crater decal (elevation-aware & scaled by blast radius)
        let crater_scale = if dest.is_building {
            3.6
        } else {
            (dest.radius * 0.95).clamp(1.25, 3.8)
        };
        commands.spawn((
            ScorchCraterVfx {
                age: 0.0,
                lifetime: 32.0,
                base_scale: crater_scale,
            },
            Mesh3d(assets.crater_mesh.clone()),
            MeshMaterial3d(assets.crater_mat.clone()),
            Transform::from_xyz(dest.position[0], dest.position[1] + 0.015, dest.position[2])
                .with_rotation(Quat::from_rotation_y(dest.entity_id as f32 * 0.7))
                .with_scale(Vec3::new(crater_scale, 1.0, crater_scale)),
        ));

        // High-intensity dynamic explosion PointLight
        commands.spawn((
            DynamicFlashLightVfx {
                age: 0.0,
                lifetime: if dest.is_building { 0.70 } else { 0.45 },
                initial_intensity: if dest.is_building { 320_000.0 } else { 150_000.0 },
            },
            PointLight {
                color: Color::srgb(1.0, 0.58, 0.18),
                intensity: if dest.is_building { 320_000.0 } else { 150_000.0 },
                range: if dest.is_building { 26.0 } else { 16.0 },
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(center + Vec3::Y * 1.6),
        ));

        // Flying fire & smoke debris burst
        let debris_count = if dest.is_building { 22 } else { 12 };
        for i in 0..debris_count {
            let a = (i as f32 / debris_count as f32) * std::f32::consts::TAU;
            let spd = 4.5 + (i % 5) as f32 * 2.1;
            let vy = 5.5 + (i % 4) as f32 * 2.4;
            let is_smoke = i % 3 == 0;
            commands.spawn((
                ParticleDebrisVfx {
                    velocity: Vec3::new(a.cos() * spd, vy, a.sin() * spd),
                    age: 0.0,
                    lifetime: if is_smoke { 1.1 } else { 0.68 },
                    gravity: if is_smoke { -1.2 } else { 18.0 },
                    initial_scale: if dest.is_building { 1.35 } else { 0.95 },
                },
                Mesh3d(if is_smoke {
                    assets.smoke_mesh.clone()
                } else {
                    assets.spark_mesh.clone()
                }),
                MeshMaterial3d(if is_smoke {
                    assets.smoke_mat.clone()
                } else {
                    assets.fire_mat.clone()
                }),
                Transform::from_translation(center),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_and_draw_vfx(
    mut commands: Commands,
    time: Res<Time>,
    mut gizmos: Gizmos,
    mut proj_q: Query<
        (Entity, &mut ProjectileVfx, &mut Transform),
        (
            Without<ParticleDebrisVfx>,
            Without<ExplosionCoreVfx>,
            Without<ScorchCraterVfx>,
        ),
    >,
    mut debris_q: Query<
        (Entity, &mut ParticleDebrisVfx, &mut Transform),
        (
            Without<ProjectileVfx>,
            Without<ExplosionCoreVfx>,
            Without<ScorchCraterVfx>,
        ),
    >,
    mut core_q: Query<
        (Entity, &mut ExplosionCoreVfx, &mut Transform),
        (
            Without<ProjectileVfx>,
            Without<ParticleDebrisVfx>,
            Without<ScorchCraterVfx>,
        ),
    >,
    mut crater_q: Query<
        (Entity, &mut ScorchCraterVfx, &mut Transform),
        (
            Without<ProjectileVfx>,
            Without<ParticleDebrisVfx>,
            Without<ExplosionCoreVfx>,
        ),
    >,
    mut light_q: Query<(Entity, &mut DynamicFlashLightVfx, &mut PointLight)>,
    mut beam_q: Query<(Entity, &mut EnergyBeamVfx)>,
    mut ring_q: Query<(Entity, &mut ShockwaveRingVfx)>,
    mut cmd_marker_q: Query<(Entity, &mut CommandMarkerVfx)>,
    mut flipbook_q: Query<(Entity, &mut VfxFlipbookBillboard)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();

    // 0. Advance RA2 VFX Flipbook Billboards
    for (ent, mut fb) in &mut flipbook_q {
        fb.age += dt;
        if fb.age >= fb.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let ratio = (fb.age / fb.lifetime).clamp(0.0, 0.999);
        let frame_idx = (ratio * fb.frame_count as f32).floor() as u32;
        let col_idx = frame_idx % fb.columns.max(1);
        let row_idx = (frame_idx / fb.columns.max(1)) % fb.rows.max(1);
        let inv_c = 1.0 / fb.columns.max(1) as f32;
        let inv_r = 1.0 / fb.rows.max(1) as f32;
        if let Some(mat) = materials.get_mut(&fb.mat_handle) {
            mat.uv_transform = bevy::math::Affine2::from_scale_angle_translation(
                Vec2::new(inv_c, inv_r),
                0.0,
                Vec2::new(col_idx as f32 * inv_c, row_idx as f32 * inv_r),
            );
        }
    }

    // 1. Advance 3D ballistic projectiles
    for (ent, mut proj, mut tf) in &mut proj_q {
        let total_dist = proj.start.distance(proj.end).max(0.1);
        proj.progress += (proj.speed * dt) / total_dist;
        if proj.progress >= 1.0 {
            commands.entity(ent).despawn();
        } else {
            let p = proj.progress;
            let base_pos = proj.start.lerp(proj.end, p);
            let arc_y = 4.0 * proj.arc_height * p * (1.0 - p);
            tf.translation = base_pos + Vec3::Y * arc_y;
        }
    }

    // 2. Advance 3D debris & smoke particles
    for (ent, mut deb, mut tf) in &mut debris_q {
        deb.age += dt;
        if deb.age >= deb.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        deb.velocity.y -= deb.gravity * dt;
        tf.translation += deb.velocity * dt;
        if tf.translation.y < 0.12 && deb.gravity > 0.0 {
            tf.translation.y = 0.12;
            deb.velocity.y = -deb.velocity.y * 0.35;
            deb.velocity.x *= 0.7;
            deb.velocity.z *= 0.7;
        }
        let life_ratio = (1.0 - deb.age / deb.lifetime).clamp(0.05, 1.0);
        let scale = if deb.gravity < 0.0 {
            // Smoke expands as it rises
            deb.initial_scale * (0.6 + (1.0 - life_ratio) * 1.4)
        } else {
            deb.initial_scale * life_ratio
        };
        tf.scale = Vec3::splat(scale);
    }

    // 3. Advance expanding 3D fireball cores
    for (ent, mut core, mut tf) in &mut core_q {
        core.age += dt;
        if core.age >= core.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let t = core.age / core.lifetime;
        let radius = if t < 0.35 {
            core.max_radius * (t / 0.35)
        } else {
            core.max_radius * (1.0 - (t - 0.35) / 0.65).max(0.05)
        };
        tf.scale = Vec3::splat(radius);
    }

    // 4. Advance dynamic point lights
    for (ent, mut flash, mut light) in &mut light_q {
        flash.age += dt;
        if flash.age >= flash.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let fade = (1.0 - flash.age / flash.lifetime).powi(2);
        light.intensity = flash.initial_intensity * fade;
    }

    // 5. Render decaying Prism Laser Beams & jagged 3D Tesla Lightning Arcs
    for (ent, mut beam) in &mut beam_q {
        beam.age += dt;
        if beam.age >= beam.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let alpha = (1.0 - beam.age / beam.lifetime).clamp(0.0, 1.0);
        if beam.is_tesla {
            // 6-segment jagged lightning arc
            let segments = 6;
            let mut prev = beam.start;
            for s in 1..=segments {
                let frac = s as f32 / segments as f32;
                let mut pt = beam.start.lerp(beam.end, frac);
                if s < segments {
                    let h = beam
                        .seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(s as u64 * 1442695040888963407);
                    let jx = (((h & 0xff) as f32 / 255.0) - 0.5) * 1.1;
                    let jy = ((((h >> 8) & 0xff) as f32 / 255.0) - 0.3) * 0.9;
                    let jz = ((((h >> 16) & 0xff) as f32 / 255.0) - 0.5) * 1.1;
                    pt += Vec3::new(jx, jy, jz);
                }
                gizmos.line(prev, pt, Color::srgba(0.45, 0.92, 1.0, alpha));
                gizmos.line(
                    prev + Vec3::Y * 0.06,
                    pt + Vec3::Y * 0.06,
                    Color::srgba(0.85, 0.98, 1.0, alpha),
                );
                prev = pt;
            }
        } else {
            let (col_main, col_outer) = match beam.style {
                ProjectileStyle::Radiation => (
                    Color::srgba(0.25, 1.0, 0.18, alpha),
                    Color::srgba(0.65, 1.0, 0.35, alpha),
                ),
                ProjectileStyle::MindControl => (
                    Color::srgba(0.88, 0.22, 1.0, alpha),
                    Color::srgba(1.0, 0.55, 0.98, alpha),
                ),
                ProjectileStyle::Sonic => (
                    Color::srgba(0.65, 0.92, 1.0, alpha),
                    Color::srgba(0.95, 1.0, 1.0, alpha),
                ),
                _ => (
                    Color::srgba(0.30, 0.98, 1.0, alpha),
                    Color::srgba(0.98, 0.38, 1.0, alpha),
                ),
            };
            gizmos.line(beam.start, beam.end, col_main);
            gizmos.line(
                beam.start + Vec3::Y * 0.10,
                beam.end + Vec3::Y * 0.10,
                col_outer,
            );
            gizmos.line(
                beam.start - Vec3::Y * 0.08,
                beam.end - Vec3::Y * 0.08,
                Color::srgba(0.85, 1.0, 1.0, alpha),
            );
        }
    }

    // 6. Render expanding ground shockwave rings
    for (ent, mut ring) in &mut ring_q {
        ring.age += dt;
        if ring.age >= ring.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let progress = (ring.age / ring.lifetime).clamp(0.0, 1.0);
        let radius = 0.3 + ring.max_radius * progress;
        let srgba = ring.color.to_srgba();
        let faded = Color::srgba(srgba.red, srgba.green, srgba.blue, 1.0 - progress);
        gizmos.circle(
            Isometry3d::new(
                ring.center,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            radius,
            faded,
        );
    }

    // 7. Render tactical 3D command confirmation rings (Move / Attack / Rally)
    for (ent, mut marker) in &mut cmd_marker_q {
        marker.age += dt;
        if marker.age >= marker.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let p = (marker.age / marker.lifetime).clamp(0.0, 1.0);
        let outer_r = (1.65 * (1.0 - p * 0.72)).max(0.2);
        let inner_r = (0.35 + p * 0.95).max(0.15);
        let c = marker.color.to_srgba();
        let col = Color::srgba(c.red, c.green, c.blue, 1.0 - p);
        gizmos.circle(
            Isometry3d::new(
                marker.center,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            outer_r,
            col,
        );
        gizmos.circle(
            Isometry3d::new(
                marker.center + Vec3::Y * 0.04,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            inner_r,
            col,
        );
    }

    // 8. Fade out ground scorch craters over time
    for (ent, mut crater, mut tf) in &mut crater_q {
        crater.age += dt;
        if crater.age >= crater.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        if crater.age > crater.lifetime * 0.7 {
            let fade = 1.0 - (crater.age - crater.lifetime * 0.7) / (crater.lifetime * 0.3);
            let s = (crater.base_scale * fade).max(0.05);
            tf.scale = Vec3::new(s, 1.0, s);
        }
    }
}
