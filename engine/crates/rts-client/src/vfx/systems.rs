use super::*;
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{CachedFogVisibility, SocketRegistry};
use crate::SimWorldResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{DeliveryKind, ProjectileStyle};
use rts_simulation::world::{EntityKind, SimWorld};

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

        let resolved_muzzle = socket_reg
            .as_deref_mut()
            .and_then(|reg| reg.next_muzzle_pos(ev.attacker_id));
        let has_exact_muzzle = resolved_muzzle.is_some();
        let start = resolved_muzzle.unwrap_or_else(|| {
            Vec3::new(
                ev.attacker_pos[0],
                ev.attacker_pos[1] + 1.15,
                ev.attacker_pos[2],
            )
        });
        let end = Vec3::new(ev.target_pos[0], ev.target_pos[1] + 0.95, ev.target_pos[2]);
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

        if is_beam {
            let beam_origin = if has_exact_muzzle {
                start
            } else if is_tesla {
                start + Vec3::Y * 1.1
            } else {
                start + Vec3::Y * 0.45
            };
            commands.spawn((
                BattlefieldVfxEntity,
                EnergyBeamVfx {
                    start: beam_origin,
                    end,
                    is_tesla,
                    style: ev.projectile_style,
                    age: 0.0,
                    lifetime: 0.32,
                    seed: world.tick.wrapping_add(idx as u64 * 97),
                },
            ));
            let flash_col = match ev.projectile_style {
                ProjectileStyle::Radiation => Color::srgb(0.25, 1.0, 0.20),
                ProjectileStyle::MindControl => Color::srgb(0.90, 0.25, 1.0),
                ProjectileStyle::Sonic => Color::srgb(0.55, 0.90, 1.0),
                _ => Color::srgb(0.3, 0.92, 1.0),
            };
            commands.spawn((
                BattlefieldVfxEntity,
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
            let uses_sim_projectile = loaded.weapons.get(&ev.weapon_id).is_some_and(|w| {
                matches!(w.delivery, DeliveryKind::Ballistic | DeliveryKind::Missile)
            }) || world
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
        }

        let spark_count = if ev.damage >= 50.0 { 9 } else { 4 };
        let spark_col = if is_beam {
            [0.35, 0.95, 1.0, 0.96]
        } else {
            [1.0, 0.56, 0.14, 0.96]
        };
        if let Some(pool) = gpu_pool.as_deref_mut() {
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
                    color_rgba: spark_col,
                });
            }
        }
    }

    for dest in &world.destruction_events {
        let non_hostile = dest.player_id == pov_pid
            || (dest.player_id != 0 && !world.are_hostile(pov_pid, dest.player_id));
        let visible_dest = non_hostile || cached_fow.is_world_pos_visible(world, dest.position);

        if !dest.is_sold && dest.player_id == pov_pid {
            interaction.push_eva_alert(if dest.is_building {
                "EVA: Structure lost!"
            } else {
                "EVA: Unit lost."
            });
        }
        if !visible_dest {
            continue;
        }

        let center = Vec3::new(
            dest.position[0],
            dest.position[1].max(0.0) + 0.9,
            dest.position[2],
        );
        let max_r = dest.radius;

        if dest.is_sold {
            commands.spawn((
                BattlefieldVfxEntity,
                ShockwaveRingVfx {
                    center: Vec3::new(
                        dest.position[0],
                        dest.position[1].max(0.0) + 0.22,
                        dest.position[2],
                    ),
                    max_radius: 4.0,
                    age: 0.0,
                    lifetime: 0.85,
                    color: Color::srgb(1.0, 0.86, 0.22),
                },
            ));
            for i in 0..14 {
                let a = (i as f32 / 14.0) * std::f32::consts::TAU;
                let r = 1.2 + (i % 3) as f32 * 0.55;
                let mat = if i % 2 == 0 {
                    assets.shell_mat.clone()
                } else {
                    assets.plasma_cyan_mat.clone()
                };
                commands.spawn((
                    BattlefieldVfxEntity,
                    ParticleDebrisVfx {
                        velocity: Vec3::new(a.cos() * 1.4, 3.8 + (i % 3) as f32, a.sin() * 1.4),
                        age: 0.0,
                        lifetime: 0.72,
                        gravity: 4.5,
                        initial_scale: 0.68,
                    },
                    Mesh3d(assets.spark_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(
                        dest.position[0] + a.cos() * r,
                        dest.position[1].max(0.0) + 1.1,
                        dest.position[2] + a.sin() * r,
                    ),
                ));
            }
            continue;
        }

        if let Ok(mut rig) = camera_q.get_single_mut() {
            rig.add_trauma(center, if dest.is_building { 0.48 } else { 0.26 });
        }
        if dest.player_id != pov_pid && dest.is_building {
            interaction.push_eva_alert("EVA: Enemy structure destroyed!");
        }

        commands.spawn((
            BattlefieldVfxEntity,
            ExplosionCoreVfx {
                age: 0.0,
                lifetime: if dest.is_building { 0.65 } else { 0.42 },
                max_radius: max_r,
            },
            Mesh3d(assets.fireball_mesh.clone()),
            MeshMaterial3d(assets.fire_mat.clone()),
            Transform::from_translation(center).with_scale(Vec3::splat(0.3)),
        ));

        let requested_key = if dest.is_building {
            "explold"
        } else if max_r >= 2.2 {
            "explomed"
        } else {
            "explosml"
        };
        let resolved_sheet = assets
            .flipbook_sheets
            .get(requested_key)
            .map(|v| (requested_key.to_string(), v.clone()))
            .or_else(|| {
                assets
                    .flipbook_sheets
                    .iter()
                    .next()
                    .map(|(k, v)| (k.clone(), v.clone()))
            });
        if let Some((actual_key, (sheet_img, f_count, cols, rows))) = resolved_sheet {
            let (frames, c_cnt, r_cnt) = (f_count.max(1), cols.max(1), rows.max(1));
            let mat_h = if let Some(pool) = gpu_pool.as_deref_mut() {
                pool.shared_flipbook_frame_material(
                    materials,
                    &actual_key,
                    &sheet_img,
                    frames,
                    c_cnt,
                    r_cnt,
                    0,
                )
            } else {
                assets.fire_mat.clone()
            };
            let b_scale = if dest.is_building { 4.2 } else { 2.6 };
            commands.spawn((
                BattlefieldVfxEntity,
                VfxFlipbookBillboard {
                    sheet_key: actual_key,
                    sheet_img,
                    age: 0.0,
                    lifetime: (frames as f32 / 18.0).clamp(0.35, 1.15),
                    frame_count: frames,
                    columns: c_cnt,
                    rows: r_cnt,
                    max_scale: b_scale,
                    current_frame: 0,
                    mat_handle: mat_h.clone(),
                },
                Mesh3d(assets.billboard_mesh.clone()),
                MeshMaterial3d(mat_h),
                Transform::from_translation(center + Vec3::Y * 0.65)
                    .with_rotation(Quat::from_rotation_x(-0.55))
                    .with_scale(Vec3::splat(b_scale)),
            ));
        }

        commands.spawn((
            BattlefieldVfxEntity,
            ShockwaveRingVfx {
                center: Vec3::new(
                    dest.position[0],
                    dest.position[1].max(0.0) + 0.16,
                    dest.position[2],
                ),
                max_radius: max_r * 2.4,
                age: 0.0,
                lifetime: if dest.is_building { 0.75 } else { 0.48 },
                color: Color::srgb(1.0, 0.68, 0.18),
            },
        ));

        let crater_scale = if dest.is_building {
            3.6
        } else {
            (dest.radius * 0.95).clamp(1.25, 3.8)
        };
        commands.spawn((
            BattlefieldVfxEntity,
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

        let flash_int = if dest.is_building {
            320_000.0
        } else {
            150_000.0
        };
        commands.spawn((
            BattlefieldVfxEntity,
            DynamicFlashLightVfx {
                age: 0.0,
                lifetime: if dest.is_building { 0.70 } else { 0.45 },
                initial_intensity: flash_int,
            },
            PointLight {
                color: Color::srgb(1.0, 0.58, 0.18),
                intensity: flash_int,
                range: if dest.is_building { 26.0 } else { 16.0 },
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(center + Vec3::Y * 1.6),
        ));

        let particle_count = if dest.is_building { 28 } else { 16 };
        if let Some(pool) = gpu_pool.as_deref_mut() {
            for i in 0..particle_count {
                let a = (i as f32 / particle_count as f32) * std::f32::consts::TAU;
                let spd = 4.2 + (i % 5) as f32 * 2.1;
                let vy = 5.2 + (i % 4) as f32 * 2.3;
                let is_smoke = i % 3 == 0;
                pool.burst_buffer.emit(GpuParticleInstance {
                    position: center,
                    velocity: Vec3::new(a.cos() * spd, vy, a.sin() * spd),
                    age: 0.0,
                    lifetime: if is_smoke { 1.15 } else { 0.72 },
                    gravity: if is_smoke { -1.2 } else { 18.0 },
                    start_size: if dest.is_building { 0.75 } else { 0.50 },
                    end_size: if is_smoke { 1.65 } else { 0.14 },
                    color_rgba: if is_smoke {
                        [0.18, 0.19, 0.21, 0.75]
                    } else {
                        [1.0, 0.52, 0.14, 0.96]
                    },
                });
            }
        }
        let shrapnel_count = if dest.is_building { 6 } else { 3 };
        for i in 0..shrapnel_count {
            let a = (i as f32 / shrapnel_count as f32) * std::f32::consts::TAU + 0.4;
            let spd = 5.0 + (i % 3) as f32 * 2.2;
            commands.spawn((
                BattlefieldVfxEntity,
                ParticleDebrisVfx {
                    velocity: Vec3::new(a.cos() * spd, 6.8 + i as f32 * 1.4, a.sin() * spd),
                    age: 0.0,
                    lifetime: 0.85,
                    gravity: 19.0,
                    initial_scale: if dest.is_building { 1.15 } else { 0.82 },
                },
                Mesh3d(assets.spark_mesh.clone()),
                MeshMaterial3d(assets.fire_mat.clone()),
                Transform::from_translation(center),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_and_draw_vfx(
    mut commands: Commands,
    time: Res<Time>,
    sim_res: Option<Res<SimWorldResource>>,
    (_vfx_assets, mut gpu_pool, mut meshes, mut images, mut materials, mut shader_mats): (
        Option<ResMut<VfxAssets>>,
        Option<ResMut<GpuParticlePoolResources>>,
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<Image>>>,
        ResMut<Assets<StandardMaterial>>,
        Option<ResMut<Assets<GpuParticleShaderMaterial>>>,
    ),
    batch_q: Query<Entity, With<VfxDynamicBatchEntity>>,
    particle_entity_q: Query<Entity, With<VfxGpuParticleBillboardEntity>>,
    camera_q: Query<
        &Transform,
        (
            With<RtsCameraRig>,
            Without<ProjectileVfx>,
            Without<ParticleDebrisVfx>,
            Without<ExplosionCoreVfx>,
            Without<ScorchCraterVfx>,
        ),
    >,
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
    mut light_q: Query<
        (Entity, &mut DynamicFlashLightVfx, &mut PointLight),
        (
            Without<crate::rendering::BuildingPointLight>,
            Without<crate::rendering::PreviewStageLight>,
        ),
    >,
    mut beam_q: Query<(Entity, &mut EnergyBeamVfx)>,
    mut ring_q: Query<(Entity, &mut ShockwaveRingVfx)>,
    mut cmd_marker_q: Query<(Entity, &mut CommandMarkerVfx)>,
    mut flipbook_q: Query<(
        Entity,
        &mut VfxFlipbookBillboard,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let dt = time.delta_secs();
    let now_secs = time.elapsed_secs();
    let grid_ref = sim_res.as_ref().map(|sr| &sr.0.grid);

    if let (Some(pool), Some(mesh_assets)) = (gpu_pool.as_deref_mut(), meshes.as_deref_mut()) {
        pool.ensure_initialized(
            &mut commands,
            mesh_assets,
            &mut materials,
            shader_mats.as_deref_mut(),
            images.as_deref_mut(),
            batch_q.is_empty(),
            particle_entity_q.is_empty(),
        );
    }

    for (ent, mut fb, mut mat_3d) in &mut flipbook_q {
        fb.age += dt;
        if fb.age >= fb.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let ratio = (fb.age / fb.lifetime).clamp(0.0, 0.999);
        let frame_idx = (ratio * fb.frame_count as f32).floor() as u32;
        if frame_idx != fb.current_frame {
            fb.current_frame = frame_idx;
            if let Some(pool) = gpu_pool.as_deref_mut() {
                let next_h = pool.shared_flipbook_frame_material(
                    &mut materials,
                    &fb.sheet_key,
                    &fb.sheet_img,
                    fb.frame_count,
                    fb.columns,
                    fb.rows,
                    frame_idx,
                );
                fb.mat_handle = next_h.clone();
                mat_3d.0 = next_h;
            }
        }
    }

    for (ent, mut proj, mut tf) in &mut proj_q {
        let total_dist = proj.start.distance(proj.end).max(0.1);
        proj.progress += (proj.speed * dt) / total_dist;
        if proj.progress >= 1.0 {
            commands.entity(ent).despawn();
        } else {
            let p = proj.progress;
            tf.translation =
                proj.start.lerp(proj.end, p) + Vec3::Y * (4.0 * proj.arc_height * p * (1.0 - p));
        }
    }

    for (ent, mut deb, mut tf) in &mut debris_q {
        deb.age += dt;
        if deb.age >= deb.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        deb.velocity.y -= deb.gravity * dt;
        tf.translation += deb.velocity * dt;
        let floor_y = grid_ref
            .map(|g| g.sample_world_height(tf.translation.x, tf.translation.z) + 0.08)
            .unwrap_or(0.12);
        if tf.translation.y < floor_y && deb.gravity > 0.0 {
            tf.translation.y = floor_y;
            deb.velocity.y = -deb.velocity.y * 0.35;
            deb.velocity.x *= 0.7;
            deb.velocity.z *= 0.7;
        }
        let life_ratio = (1.0 - deb.age / deb.lifetime).clamp(0.05, 1.0);
        let scale = if deb.gravity < 0.0 {
            deb.initial_scale * (0.6 + (1.0 - life_ratio) * 1.4)
        } else {
            deb.initial_scale * life_ratio
        };
        tf.scale = Vec3::splat(scale);
    }

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

    for (ent, mut flash, mut light) in &mut light_q {
        flash.age += dt;
        if flash.age >= flash.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        light.intensity = flash.initial_intensity * (1.0 - flash.age / flash.lifetime).powi(2);
    }

    let (cam_forward, cam_right, cam_up) = camera_q
        .get_single()
        .map(|tf| {
            (
                tf.forward().as_vec3(),
                tf.right().as_vec3(),
                tf.up().as_vec3(),
            )
        })
        .unwrap_or_else(|_| {
            (
                Vec3::new(0.0, -0.75, -0.66).normalize(),
                Vec3::X,
                Vec3::new(0.0, 0.66, -0.75).normalize(),
            )
        });
    let mut batch = VfxBeamAndRingBatchBuilder::default();

    for (ent, mut beam) in &mut beam_q {
        beam.age += dt;
        if beam.age >= beam.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let alpha = (1.0 - beam.age / beam.lifetime).clamp(0.0, 1.0);
        if beam.is_tesla {
            let segments = 6usize;
            let mut prev = beam.start;
            for s in 1..=segments {
                let mut pt = beam.start.lerp(beam.end, s as f32 / segments as f32);
                if s < segments {
                    let h = beam
                        .seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(s as u64 * 1442695040888963407);
                    pt += Vec3::new(
                        (((h & 0xff) as f32 / 255.0) - 0.5) * 1.1,
                        ((((h >> 8) & 0xff) as f32 / 255.0) - 0.3) * 0.9,
                        ((((h >> 16) & 0xff) as f32 / 255.0) - 0.5) * 1.1,
                    );
                }
                batch.push_3d_beam_segment(
                    prev,
                    pt,
                    cam_forward,
                    0.10,
                    [0.38, 0.88, 1.0, alpha * 0.85],
                );
                batch.push_3d_beam_segment(prev, pt, cam_forward, 0.038, [0.92, 0.99, 1.0, alpha]);
                prev = pt;
            }
        } else {
            let (col_outer, col_core) = match beam.style {
                ProjectileStyle::Radiation => {
                    ([0.25, 1.0, 0.18, alpha * 0.78], [0.78, 1.0, 0.52, alpha])
                }
                ProjectileStyle::MindControl => {
                    ([0.88, 0.22, 1.0, alpha * 0.80], [1.0, 0.72, 1.0, alpha])
                }
                ProjectileStyle::Sonic => {
                    ([0.58, 0.92, 1.0, alpha * 0.80], [0.96, 1.0, 1.0, alpha])
                }
                _ => ([0.28, 0.96, 1.0, alpha * 0.82], [0.98, 0.85, 1.0, alpha]),
            };
            batch.push_3d_beam_segment(beam.start, beam.end, cam_forward, 0.12, col_outer);
            batch.push_3d_beam_segment(beam.start, beam.end, cam_forward, 0.045, col_core);
        }
    }

    for (ent, mut ring) in &mut ring_q {
        ring.age += dt;
        if ring.age >= ring.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let progress = (ring.age / ring.lifetime).clamp(0.0, 1.0);
        let srgba = ring.color.to_srgba();
        batch.push_ground_ring(
            ring.center,
            0.3 + ring.max_radius * progress,
            0.16 * (1.0 - progress * 0.45),
            24,
            [
                srgba.red,
                srgba.green,
                srgba.blue,
                (1.0 - progress).clamp(0.0, 1.0),
            ],
        );
    }

    for (ent, mut marker) in &mut cmd_marker_q {
        marker.age += dt;
        if marker.age >= marker.lifetime {
            commands.entity(ent).despawn();
            continue;
        }
        let p = (marker.age / marker.lifetime).clamp(0.0, 1.0);
        let c = marker.color.to_srgba();
        let col = [c.red, c.green, c.blue, (1.0 - p).clamp(0.0, 1.0)];
        batch.push_ground_ring(
            marker.center,
            (1.65 * (1.0 - p * 0.72)).max(0.2),
            0.09,
            20,
            col,
        );
        batch.push_ground_ring(
            marker.center + Vec3::Y * 0.03,
            (0.35 + p * 0.95).max(0.15),
            0.07,
            16,
            col,
        );
    }

    if let (Some(pool), Some(mesh_assets)) = (gpu_pool.as_deref_mut(), meshes.as_deref_mut()) {
        if let Some(m) = mesh_assets.get_mut(&pool.batch_mesh) {
            batch.write_into_mesh(m);
        }
        let part_mesh_handle = pool.particle_mesh.clone();
        if let Some(pm) = mesh_assets.get_mut(&part_mesh_handle) {
            if pool.uses_gpu_shader {
                pool.burst_buffer
                    .step_and_upload_gpu_shader_particles(now_secs, dt, pm);
            } else {
                pool.burst_buffer
                    .step_and_build_billboards(dt, grid_ref, cam_right, cam_up, pm);
            }
        }
    }

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
