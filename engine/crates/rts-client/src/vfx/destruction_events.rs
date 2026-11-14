use super::{
    BattlefieldVfxEntity, DynamicFlashLightVfx, ExplosionCoreVfx, GpuParticleInstance,
    GpuParticlePoolResources, ParticleDebrisVfx, ScorchCraterVfx, ShockwaveRingVfx, VfxAssets,
    VfxFlipbookBillboard,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::CachedFogVisibility;
use bevy::prelude::*;
use rts_simulation::world::SimWorld;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_destruction_vfx(
    commands: &mut Commands,
    world: &SimWorld,
    cached_fow: &CachedFogVisibility,
    pov_pid: u8,
    assets: &mut VfxAssets,
    mut gpu_pool: Option<&mut GpuParticlePoolResources>,
    materials: &mut Assets<StandardMaterial>,
    interaction: &mut ClientInteractionState,
    camera_q: &mut Query<&mut RtsCameraRig>,
) {
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
