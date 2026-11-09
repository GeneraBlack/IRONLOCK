mod systems;
pub use systems::update_and_draw_vfx;
use systems::spawn_combat_and_destruction_events;

use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::SimWorldResource;
use bevy::prelude::*;
use rts_sdk::schema::ProjectileStyle;
use rts_simulation::world::{EntityKind, HarvesterPhase, VeterancyRank};
use std::collections::BTreeMap;

#[derive(Resource)]
pub struct VfxAssets {
    pub shell_mesh: Handle<Mesh>,
    pub spark_mesh: Handle<Mesh>,
    pub fireball_mesh: Handle<Mesh>,
    pub smoke_mesh: Handle<Mesh>,
    pub crater_mesh: Handle<Mesh>,
    pub billboard_mesh: Handle<Mesh>,
    pub shell_mat: Handle<StandardMaterial>,
    pub fire_mat: Handle<StandardMaterial>,
    pub plasma_cyan_mat: Handle<StandardMaterial>,
    pub smoke_mat: Handle<StandardMaterial>,
    pub crater_mat: Handle<StandardMaterial>,
    pub flipbook_sheets: BTreeMap<String, (Handle<Image>, u32, u32, u32)>,
}

#[derive(Resource, Default)]
pub struct VfxTrackerState {
    pub last_sim_tick: u64,
    pub smoke_timer: f32,
    pub under_attack_cooldown: f32,
    pub known_veterancy: BTreeMap<u64, VeterancyRank>,
    pub known_construction_done: BTreeMap<u64, bool>,
}

#[derive(Component)]
pub struct ProjectileVfx {
    pub start: Vec3,
    pub end: Vec3,
    pub progress: f32,
    pub speed: f32,
    pub arc_height: f32,
}

#[derive(Component)]
pub struct ParticleDebrisVfx {
    pub velocity: Vec3,
    pub age: f32,
    pub lifetime: f32,
    pub gravity: f32,
    pub initial_scale: f32,
}

#[derive(Component)]
pub struct ExplosionCoreVfx {
    pub age: f32,
    pub lifetime: f32,
    pub max_radius: f32,
}

#[derive(Component)]
pub struct DynamicFlashLightVfx {
    pub age: f32,
    pub lifetime: f32,
    pub initial_intensity: f32,
}

#[derive(Component)]
pub struct EnergyBeamVfx {
    pub start: Vec3,
    pub end: Vec3,
    pub is_tesla: bool,
    pub style: ProjectileStyle,
    pub age: f32,
    pub lifetime: f32,
    pub seed: u64,
}

#[derive(Component)]
pub struct ShockwaveRingVfx {
    pub center: Vec3,
    pub max_radius: f32,
    pub age: f32,
    pub lifetime: f32,
    pub color: Color,
}

#[derive(Component)]
pub struct CommandMarkerVfx {
    pub center: Vec3,
    pub age: f32,
    pub lifetime: f32,
    pub color: Color,
}

#[derive(Component)]
pub struct ScorchCraterVfx {
    pub age: f32,
    pub lifetime: f32,
    pub base_scale: f32,
}

#[derive(Component)]
pub struct VfxFlipbookBillboard {
    pub age: f32,
    pub lifetime: f32,
    pub frame_count: u32,
    pub columns: u32,
    pub rows: u32,
    pub max_scale: f32,
    pub mat_handle: Handle<StandardMaterial>,
}

pub fn setup_vfx_assets(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    mod_res: Option<Res<crate::ModResource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let shell_mesh = meshes.add(Cuboid::new(0.16, 0.16, 0.58));
    let spark_mesh = meshes.add(Cuboid::new(0.20, 0.20, 0.20));
    let fireball_mesh = meshes.add(Sphere::new(0.65));
    let smoke_mesh = meshes.add(Sphere::new(0.38));
    let crater_mesh = meshes.add(Cuboid::new(1.0, 0.02, 1.0));
    let billboard_mesh = meshes.add(Rectangle::new(1.0, 1.0));

    let mut flipbook_sheets = BTreeMap::new();
    if let (Some(srv), Some(mres)) = (asset_server.as_deref(), mod_res.as_deref()) {
        for (vfx_id, def) in &mres.0.vfx_catalog {
            let qual = crate::input::qualify_mod_asset_rel_path(&mres.0, &def.sprite_sheet);
            let h: Handle<Image> = if !std::path::Path::new(&qual).exists()
                && std::path::Path::new("rust-ra2").join(&qual).exists()
            {
                srv.load(format!("rust-ra2/{qual}"))
            } else {
                srv.load(qual)
            };
            flipbook_sheets.insert(
                vfx_id.to_ascii_lowercase(),
                (
                    h,
                    def.exported_frames.max(1),
                    def.columns.max(1),
                    def.rows.max(1),
                ),
            );
        }
    }

    let shell_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.35),
        emissive: LinearRgba::new(4.5, 2.8, 0.45, 1.0),
        unlit: true,
        ..default()
    });
    let fire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.48, 0.12),
        emissive: LinearRgba::new(5.5, 1.8, 0.25, 1.0),
        unlit: true,
        ..default()
    });
    let plasma_cyan_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.95, 1.0),
        emissive: LinearRgba::new(0.6, 4.5, 6.0, 1.0),
        unlit: true,
        ..default()
    });
    let smoke_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.19, 0.21),
        perceptual_roughness: 0.95,
        ..default()
    });
    let crater_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.08, 0.07),
        perceptual_roughness: 0.98,
        ..default()
    });

    commands.insert_resource(VfxAssets {
        shell_mesh,
        spark_mesh,
        fireball_mesh,
        smoke_mesh,
        crater_mesh,
        billboard_mesh,
        shell_mat,
        fire_mat,
        plasma_cyan_mat,
        smoke_mat,
        crater_mat,
        flipbook_sheets,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_combat_and_destruction_vfx(
    mut commands: Commands,
    time: Res<Time>,
    sim_res: Res<SimWorldResource>,
    mut interaction: ResMut<ClientInteractionState>,
    vfx_assets: Option<Res<VfxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tracker: ResMut<VfxTrackerState>,
    mut camera_q: Query<&mut RtsCameraRig>,
) {
    let dt = time.delta_secs();
    tracker.under_attack_cooldown = (tracker.under_attack_cooldown - dt).max(0.0);
    for (_, rem) in &mut interaction.battle_feed {
        *rem -= dt;
    }
    interaction.battle_feed.retain(|(_, rem)| *rem > 0.0);

    for (pos, color) in interaction.pending_command_rings.drain(..) {
        commands.spawn(CommandMarkerVfx {
            center: pos,
            age: 0.0,
            lifetime: 0.45,
            color,
        });
    }
    let Some(assets) = vfx_assets else {
        return;
    };
    let world = &sim_res.0;

    // 1. Emit continuous rising smoke & fire plumes on damaged buildings, track dust for moving vehicles, and ore mining sparks
    tracker.smoke_timer += dt;
    if tracker.smoke_timer >= 0.16 {
        tracker.smoke_timer = 0.0;
        for ent in world.entities.values() {
            if ent.health <= 0.0 {
                continue;
            }
            match &ent.kind {
                EntityKind::Building { .. } => {
                    // Emit welding sparks & foundation dust puffs while a building is under construction
                    if ent.construction_progress < 0.99 {
                        let hash = ent.id.wrapping_mul(2654435761) ^ world.tick;
                        let angle = ((hash & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                        let active_y = 0.20 + ent.construction_progress * 2.15;
                        let spark_pos = Vec3::new(
                            ent.position[0] + angle.cos() * 1.15,
                            active_y,
                            ent.position[2] + angle.sin() * 1.15,
                        );
                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(
                                    angle.cos() * 2.8,
                                    3.2,
                                    angle.sin() * 2.8,
                                ),
                                age: 0.0,
                                lifetime: 0.34,
                                gravity: 11.0,
                                initial_scale: 0.58,
                            },
                            Mesh3d(assets.spark_mesh.clone()),
                            MeshMaterial3d(if hash & 1 == 0 {
                                assets.plasma_cyan_mat.clone()
                            } else {
                                assets.shell_mat.clone()
                            }),
                            Transform::from_translation(spark_pos),
                        ));

                        // Foundation dust puff
                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(angle.cos() * 0.9, 0.7, angle.sin() * 0.9),
                                age: 0.0,
                                lifetime: 0.55,
                                gravity: -0.2,
                                initial_scale: 0.50,
                            },
                            Mesh3d(assets.smoke_mesh.clone()),
                            MeshMaterial3d(assets.smoke_mat.clone()),
                            Transform::from_xyz(
                                ent.position[0] - angle.cos() * 1.4,
                                0.18,
                                ent.position[2] - angle.sin() * 1.4,
                            )
                            .with_scale(Vec3::splat(0.38)),
                        ));
                    }

                    // Emit golden tech sparkles while a building is upgrading in-place (`upgrading_to.is_some()`)
                    if ent.upgrading_to.is_some() {
                        let hash = ent.id.wrapping_mul(2246822519) ^ world.tick;
                        let angle = ((hash & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                        let spark_pos = Vec3::new(
                            ent.position[0] + angle.cos() * 1.4,
                            1.1,
                            ent.position[2] + angle.sin() * 1.4,
                        );
                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(angle.cos() * 1.8, 3.6, angle.sin() * 1.8),
                                age: 0.0,
                                lifetime: 0.38,
                                gravity: 6.0,
                                initial_scale: 0.62,
                            },
                            Mesh3d(assets.spark_mesh.clone()),
                            MeshMaterial3d(assets.shell_mat.clone()),
                            Transform::from_translation(spark_pos),
                        ));
                    }

                    let hp_ratio = ent.health / ent.max_health.max(1.0);
                    if hp_ratio < 0.65 {
                        let hash = ent.id.wrapping_mul(2654435761) ^ world.tick;
                        let ox = (((hash & 0xff) as f32 / 255.0) - 0.5) * 1.8;
                        let oz = ((((hash >> 8) & 0xff) as f32 / 255.0) - 0.5) * 1.8;
                        let spawn_pos = Vec3::new(ent.position[0] + ox, 1.8, ent.position[2] + oz);

                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(ox * 0.3, 2.4, oz * 0.3),
                                age: 0.0,
                                lifetime: 1.35,
                                gravity: -0.6,
                                initial_scale: 0.85,
                            },
                            Mesh3d(assets.smoke_mesh.clone()),
                            MeshMaterial3d(assets.smoke_mat.clone()),
                            Transform::from_translation(spawn_pos).with_scale(Vec3::splat(0.55)),
                        ));

                        if hp_ratio < 0.35 {
                            commands.spawn((
                                ParticleDebrisVfx {
                                    velocity: Vec3::new(ox * 0.8, 3.2, oz * 0.8),
                                    age: 0.0,
                                    lifetime: 0.45,
                                    gravity: 4.0,
                                    initial_scale: 0.75,
                                },
                                Mesh3d(assets.spark_mesh.clone()),
                                MeshMaterial3d(assets.fire_mat.clone()),
                                Transform::from_translation(spawn_pos),
                            ));
                        }
                    }
                }
                EntityKind::Unit { .. } => {
                    // Track dust trail behind moving vehicles
                    if !ent.path_waypoints.is_empty() && ent.max_health >= 250.0 {
                        let back_x = ent.position[0] - ent.facing_radians.sin() * 1.1;
                        let back_z = ent.position[2] - ent.facing_radians.cos() * 1.1;
                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(0.0, 0.85, 0.0),
                                age: 0.0,
                                lifetime: 0.55,
                                gravity: -0.2,
                                initial_scale: 0.42,
                            },
                            Mesh3d(assets.smoke_mesh.clone()),
                            MeshMaterial3d(assets.smoke_mat.clone()),
                            Transform::from_xyz(back_x, 0.22, back_z)
                                .with_scale(Vec3::splat(0.32)),
                        ));
                    }

                    // Condition Yellow (< 50% HP) & Condition Red (< 25% HP) damage smoke/fire on vehicles
                    let hp_ratio = ent.health / ent.max_health.max(1.0);
                    if ent.max_health >= 180.0 && hp_ratio < 0.50 {
                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(0.0, 1.9, 0.0),
                                age: 0.0,
                                lifetime: 0.95,
                                gravity: -0.4,
                                initial_scale: 0.52,
                            },
                            Mesh3d(assets.smoke_mesh.clone()),
                            MeshMaterial3d(assets.smoke_mat.clone()),
                            Transform::from_xyz(ent.position[0], ent.position[1] + 1.05, ent.position[2])
                                .with_scale(Vec3::splat(0.40)),
                        ));
                        if hp_ratio < 0.25 {
                            commands.spawn((
                                ParticleDebrisVfx {
                                    velocity: Vec3::new(0.0, 2.6, 0.0),
                                    age: 0.0,
                                    lifetime: 0.36,
                                    gravity: 3.5,
                                    initial_scale: 0.55,
                                },
                                Mesh3d(assets.spark_mesh.clone()),
                                MeshMaterial3d(assets.fire_mat.clone()),
                                Transform::from_xyz(ent.position[0], ent.position[1] + 0.95, ent.position[2]),
                            ));
                        }
                    }

                    // Subterranean burrowing dirt/dust trail when unit is tunneling underground
                    if ent.burrowed {
                        commands.spawn((
                            ParticleDebrisVfx {
                                velocity: Vec3::new(0.0, 2.2, 0.0),
                                age: 0.0,
                                lifetime: 0.45,
                                gravity: 6.5,
                                initial_scale: 0.62,
                            },
                            Mesh3d(assets.smoke_mesh.clone()),
                            MeshMaterial3d(assets.crater_mat.clone()),
                            Transform::from_xyz(ent.position[0], 0.18, ent.position[2]),
                        ));
                    }

                    // Golden ore mining sparkles when a Harvester is actively mining
                    if let Some(harv) = &ent.harvester_state {
                        if matches!(harv.phase, HarvesterPhase::Harvesting(_)) {
                            let front_x = ent.position[0] + ent.facing_radians.sin() * 1.25;
                            let front_z = ent.position[2] + ent.facing_radians.cos() * 1.25;
                            commands.spawn((
                                ParticleDebrisVfx {
                                    velocity: Vec3::new(0.0, 2.8, 0.0),
                                    age: 0.0,
                                    lifetime: 0.35,
                                    gravity: 9.0,
                                    initial_scale: 0.55,
                                },
                                Mesh3d(assets.spark_mesh.clone()),
                                MeshMaterial3d(assets.shell_mat.clone()),
                                Transform::from_xyz(front_x, 0.45, front_z),
                            ));
                        }
                    }
                }
            }
        }
        for scar in &world.battlefield_scars {
            if scar.is_wreck && scar.remaining_secs > 5.0 {
                commands.spawn((
                    ParticleDebrisVfx {
                        velocity: Vec3::new(0.15, 1.85, -0.10),
                        age: 0.0,
                        lifetime: 0.85,
                        gravity: -0.6,
                        initial_scale: 0.52,
                    },
                    Mesh3d(assets.smoke_mesh.clone()),
                    MeshMaterial3d(assets.smoke_mat.clone()),
                    Transform::from_xyz(scar.position[0], scar.position[1].max(0.25), scar.position[2]),
                ));
            }
        }
    }

    // 2. Process new simulation tick combat, veterancy promotions & destruction events
    if world.tick == tracker.last_sim_tick {
        return;
    }
    tracker.last_sim_tick = world.tick;

    // Check for unit Veterancy Promotions (Rookie -> Veteran -> Elite) & Building Construction Completion
    for ent in world.entities.values() {
        if matches!(ent.kind, EntityKind::Building { .. }) {
            let is_done = ent.construction_progress >= 1.0;
            let was_done = tracker
                .known_construction_done
                .insert(ent.id, is_done)
                .unwrap_or(is_done);
            if is_done && !was_done {
                commands.spawn(ShockwaveRingVfx {
                    center: Vec3::new(ent.position[0], 0.20, ent.position[2]),
                    max_radius: 4.2,
                    age: 0.0,
                    lifetime: 0.65,
                    color: Color::srgb(0.25, 0.98, 0.48),
                });
                if ent.player_id == interaction.local_player_id {
                    interaction.push_eva_alert("EVA: Construction complete!");
                }
            }
        }

        let prev_rank = tracker
            .known_veterancy
            .insert(ent.id, ent.veterancy)
            .unwrap_or(VeterancyRank::Rookie);
        if ent.veterancy != prev_rank && ent.veterancy != VeterancyRank::Rookie {
            let promo_pos = Vec3::new(ent.position[0], 0.45, ent.position[2]);
            commands.spawn(ShockwaveRingVfx {
                center: promo_pos,
                max_radius: 3.4,
                age: 0.0,
                lifetime: 0.65,
                color: Color::srgb(1.0, 0.88, 0.18),
            });
            if ent.player_id == interaction.local_player_id {
                interaction.push_eva_alert(format!(
                    "EVA: Unit #{} promoted to {:?} rank!",
                    ent.id, ent.veterancy
                ));
            }
        }
    }

    spawn_combat_and_destruction_events(
        &mut commands,
        world,
        &assets,
        &mut materials,
        &mut interaction,
        &mut tracker,
        &mut camera_q,
    );
}
