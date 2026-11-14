use super::signature_beams::{emit_signature_beam_geometry, VfxBeamAndRingBatchBuilder};
use super::*;
use crate::camera::RtsCameraRig;
use crate::SimWorldResource;
use bevy::prelude::*;

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
        emit_signature_beam_geometry(&beam, cam_forward, &mut batch);
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
