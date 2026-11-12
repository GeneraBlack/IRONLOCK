pub mod data_driven_vfx;
pub mod data_driven_vfx_emitters;
pub mod decal_atlas;
pub mod decals;
pub mod gpu_particles;
mod systems;
pub mod weather_vfx;

pub use data_driven_vfx::*;
pub use decal_atlas::*;
pub use decals::*;
pub use gpu_particles::*;
use systems::spawn_combat_and_destruction_events;
pub use systems::update_and_draw_vfx;
pub use weather_vfx::*;

use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{CachedFogVisibility, SocketRegistry};
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::ProjectileStyle;
use rts_simulation::world::{EntityKind, HarvesterPhase, TileVisibility, VeterancyRank};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Component)]
pub struct BattlefieldVfxEntity;

/// Live Studio 3D Preview Stage playback & override controller for `VfxEffectDef`s (`Task 244`).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct VfxPreviewStageState {
    pub active_effect_id: Option<String>,
    pub override_effect: Option<rts_sdk::schema::VfxEffectDef>,
    pub loop_enabled: bool,
    pub timeline_secs: f32,
    pub playback_paused: bool,
    pub trigger_burst: bool,
}

impl Default for VfxPreviewStageState {
    fn default() -> Self {
        Self {
            active_effect_id: None,
            override_effect: None,
            loop_enabled: true,
            timeline_secs: 0.0,
            playback_paused: false,
            trigger_burst: false,
        }
    }
}

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
    pub flipbook_material_pool: BTreeMap<String, Vec<Handle<StandardMaterial>>>,
}

impl VfxAssets {
    pub fn reload_flipbook_sheets(
        &mut self,
        srv: &AssetServer,
        asset_root: &Path,
        loaded: &LoadedMod,
    ) {
        self.flipbook_sheets.clear();
        self.flipbook_material_pool.clear();
        let norm_asset =
            std::fs::canonicalize(asset_root).unwrap_or_else(|_| asset_root.to_path_buf());
        let norm_mod =
            std::fs::canonicalize(&loaded.root_dir).unwrap_or_else(|_| loaded.root_dir.clone());
        let mod_prefix = norm_mod
            .strip_prefix(&norm_asset)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        for (vfx_id, def) in &loaded.vfx_catalog {
            let clean = def.sprite_sheet.trim_start_matches('/').replace('\\', "/");
            let load_path = if mod_prefix.is_empty() || clean.starts_with(&format!("{mod_prefix}/"))
            {
                clean
            } else {
                format!("{mod_prefix}/{clean}")
            };
            let h: Handle<Image> = srv.load(load_path);
            self.flipbook_sheets.insert(
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
}

#[derive(Resource, Default)]
pub struct VfxTrackerState {
    pub last_sim_tick: u64,
    pub smoke_timer: f32,
    pub under_attack_cooldown: f32,
    pub known_veterancy: BTreeMap<u64, VeterancyRank>,
    pub known_construction_done: BTreeMap<u64, bool>,
}

impl VfxTrackerState {
    pub fn reset(&mut self) {
        self.last_sim_tick = 0;
        self.smoke_timer = 0.0;
        self.under_attack_cooldown = 0.0;
        self.known_veterancy.clear();
        self.known_construction_done.clear();
    }
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
    pub sheet_key: String,
    pub sheet_img: Handle<Image>,
    pub age: f32,
    pub lifetime: f32,
    pub frame_count: u32,
    pub columns: u32,
    pub rows: u32,
    pub max_scale: f32,
    pub current_frame: u32,
    pub mat_handle: Handle<StandardMaterial>,
}

pub fn setup_vfx_assets(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    asset_root: Option<Res<AssetRootResource>>,
    mod_res: Option<Res<ModResource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let shell_mesh = meshes.add(Cuboid::new(0.16, 0.16, 0.58));
    let spark_mesh = meshes.add(Cuboid::new(0.20, 0.20, 0.20));
    let fireball_mesh = meshes.add(Sphere::new(0.65));
    let smoke_mesh = meshes.add(Sphere::new(0.38));
    let crater_mesh = meshes.add(Cuboid::new(1.0, 0.02, 1.0));
    let billboard_mesh = meshes.add(Rectangle::new(1.0, 1.0));

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

    let mut vfx_res = VfxAssets {
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
        flipbook_sheets: BTreeMap::new(),
        flipbook_material_pool: BTreeMap::new(),
    };
    if let (Some(srv), Some(mres)) = (asset_server.as_deref(), mod_res.as_deref()) {
        let root_path = asset_root
            .as_deref()
            .map(|r| r.0.as_path())
            .unwrap_or_else(|| Path::new("."));
        vfx_res.reload_flipbook_sheets(srv, root_path, &mres.0);
    }

    commands.insert_resource(vfx_res);
    commands.init_resource::<GpuParticlePoolResources>();
    commands.init_resource::<DataDrivenVfxRuntimeState>();
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_combat_and_destruction_vfx(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    cached_fow: Res<CachedFogVisibility>,
    mut interaction: ResMut<ClientInteractionState>,
    mut vfx_assets: Option<ResMut<VfxAssets>>,
    mut gpu_pool: Option<ResMut<GpuParticlePoolResources>>,
    mut socket_reg: Option<ResMut<SocketRegistry>>,
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
        commands.spawn((
            BattlefieldVfxEntity,
            CommandMarkerVfx {
                center: pos,
                age: 0.0,
                lifetime: 0.45,
                color,
            },
        ));
    }
    let Some(assets) = vfx_assets.as_mut() else {
        return;
    };
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let (pov_pid, _) = CachedFogVisibility::effective_pov_and_fog(world, &interaction);

    if world.tick < tracker.last_sim_tick {
        tracker.reset();
    }

    // 1. Emit continuous rising smoke & fire plumes anchored on `DamageSmoke_01`, track dust on `Exhaust_01`,
    //    and ore mining sparks into the GPU-batched `GpuParticleBurstBuffer` (zero ECS entity spawns per puff!)
    tracker.smoke_timer += dt;
    if tracker.smoke_timer >= 0.16 {
        tracker.smoke_timer = 0.0;
        for ent in world.entities.values() {
            if ent.health <= 0.0 || !cached_fow.is_entity_visible(world, loaded, ent) {
                continue;
            }
            match &ent.kind {
                EntityKind::Building { .. } => {
                    if ent.construction_progress < 0.99 {
                        let hash = ent.id.wrapping_mul(2654435761) ^ world.tick;
                        let angle = ((hash & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                        let ground_y = world
                            .grid
                            .sample_world_height(ent.position[0], ent.position[2]);
                        let active_y = ground_y + 0.20 + ent.construction_progress * 2.15;
                        let spark_pos = Vec3::new(
                            ent.position[0] + angle.cos() * 1.15,
                            active_y,
                            ent.position[2] + angle.sin() * 1.15,
                        );
                        if let Some(pool) = gpu_pool.as_deref_mut() {
                            pool.burst_buffer.emit(GpuParticleInstance {
                                position: spark_pos,
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
                            pool.burst_buffer.emit(GpuParticleInstance {
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
                        }
                    }

                    if ent.upgrading_to.is_some() {
                        let hash = ent.id.wrapping_mul(2246822519) ^ world.tick;
                        let angle = ((hash & 0xff) as f32 / 255.0) * std::f32::consts::TAU;
                        if let Some(pool) = gpu_pool.as_deref_mut() {
                            pool.burst_buffer.emit(GpuParticleInstance {
                                position: Vec3::new(
                                    ent.position[0] + angle.cos() * 1.4,
                                    ent.position[1].max(0.0) + 1.1,
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

                    let hp_ratio = ent.health / ent.max_health.max(1.0);
                    if hp_ratio < 0.65 {
                        let hash = ent.id.wrapping_mul(2654435761) ^ world.tick;
                        let ox = (((hash & 0xff) as f32 / 255.0) - 0.5) * 0.85;
                        let oz = ((((hash >> 8) & 0xff) as f32 / 255.0) - 0.5) * 0.85;
                        let anchor = socket_reg
                            .as_deref()
                            .and_then(|reg| reg.damage_smoke_pos(ent.id))
                            .unwrap_or_else(|| {
                                Vec3::new(
                                    ent.position[0],
                                    ent.position[1].max(0.0) + 1.8,
                                    ent.position[2],
                                )
                            });
                        let spawn_pos = anchor + Vec3::new(ox, 0.0, oz);
                        if let Some(pool) = gpu_pool.as_deref_mut() {
                            pool.burst_buffer.emit(GpuParticleInstance {
                                position: spawn_pos,
                                velocity: Vec3::new(ox * 0.35, 2.35, oz * 0.35),
                                age: 0.0,
                                lifetime: 1.35,
                                gravity: -0.55,
                                start_size: 0.55,
                                end_size: 1.35,
                                color_rgba: [0.20, 0.21, 0.23, 0.72],
                            });
                            if hp_ratio < 0.35 {
                                pool.burst_buffer.emit(GpuParticleInstance {
                                    position: spawn_pos,
                                    velocity: Vec3::new(ox * 0.8, 3.2, oz * 0.8),
                                    age: 0.0,
                                    lifetime: 0.46,
                                    gravity: 3.8,
                                    start_size: 0.42,
                                    end_size: 0.12,
                                    color_rgba: [1.0, 0.48, 0.12, 0.92],
                                });
                            }
                        }
                    }
                }
                EntityKind::Unit { .. } => {
                    if !ent.path_waypoints.is_empty() && ent.max_health >= 250.0 {
                        let exhaust_origin = socket_reg
                            .as_deref()
                            .and_then(|reg| reg.exhaust_pos(ent.id))
                            .unwrap_or_else(|| {
                                Vec3::new(
                                    ent.position[0] - ent.facing_radians.sin() * 1.1,
                                    ent.position[1].max(0.0) + 0.22,
                                    ent.position[2] - ent.facing_radians.cos() * 1.1,
                                )
                            });
                        if let Some(pool) = gpu_pool.as_deref_mut() {
                            pool.burst_buffer.emit(GpuParticleInstance {
                                position: exhaust_origin,
                                velocity: Vec3::new(0.0, 0.85, 0.0),
                                age: 0.0,
                                lifetime: 0.55,
                                gravity: -0.2,
                                start_size: 0.30,
                                end_size: 0.68,
                                color_rgba: [0.46, 0.43, 0.38, 0.48],
                            });
                        }
                    }

                    let hp_ratio = ent.health / ent.max_health.max(1.0);
                    if ent.max_health >= 180.0 && hp_ratio < 0.50 {
                        let dmg_origin = socket_reg
                            .as_deref()
                            .and_then(|reg| reg.damage_smoke_pos(ent.id))
                            .unwrap_or_else(|| {
                                Vec3::new(ent.position[0], ent.position[1] + 1.05, ent.position[2])
                            });
                        if let Some(pool) = gpu_pool.as_deref_mut() {
                            pool.burst_buffer.emit(GpuParticleInstance {
                                position: dmg_origin,
                                velocity: Vec3::new(0.0, 1.9, 0.0),
                                age: 0.0,
                                lifetime: 0.95,
                                gravity: -0.4,
                                start_size: 0.38,
                                end_size: 0.92,
                                color_rgba: [0.18, 0.19, 0.21, 0.68],
                            });
                            if hp_ratio < 0.25 {
                                pool.burst_buffer.emit(GpuParticleInstance {
                                    position: dmg_origin - Vec3::Y * 0.08,
                                    velocity: Vec3::new(0.0, 2.6, 0.0),
                                    age: 0.0,
                                    lifetime: 0.36,
                                    gravity: 3.5,
                                    start_size: 0.34,
                                    end_size: 0.10,
                                    color_rgba: [1.0, 0.50, 0.14, 0.92],
                                });
                            }
                        }
                    }

                    if ent.burrowed {
                        if let Some(pool) = gpu_pool.as_deref_mut() {
                            pool.burst_buffer.emit(GpuParticleInstance {
                                position: Vec3::new(
                                    ent.position[0],
                                    world
                                        .grid
                                        .sample_world_height(ent.position[0], ent.position[2])
                                        + 0.16,
                                    ent.position[2],
                                ),
                                velocity: Vec3::new(0.0, 2.2, 0.0),
                                age: 0.0,
                                lifetime: 0.45,
                                gravity: 6.5,
                                start_size: 0.38,
                                end_size: 0.56,
                                color_rgba: [0.36, 0.28, 0.18, 0.78],
                            });
                        }
                    }

                    if let Some(harv) = &ent.harvester_state {
                        if matches!(harv.phase, HarvesterPhase::Harvesting(_)) {
                            let front_x = ent.position[0] + ent.facing_radians.sin() * 1.25;
                            let front_z = ent.position[2] + ent.facing_radians.cos() * 1.25;
                            if let Some(pool) = gpu_pool.as_deref_mut() {
                                pool.burst_buffer.emit(GpuParticleInstance {
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
        }
        for scar in &world.battlefield_scars {
            if scar.is_wreck
                && scar.remaining_secs > 5.0
                && cached_fow.world_pos_visibility(world, scar.position)
                    != TileVisibility::Unexplored
            {
                let ground_y = world
                    .grid
                    .sample_world_height(scar.position[0], scar.position[2]);
                if let Some(pool) = gpu_pool.as_deref_mut() {
                    pool.burst_buffer.emit(GpuParticleInstance {
                        position: Vec3::new(scar.position[0], ground_y + 0.52, scar.position[2]),
                        velocity: Vec3::new(0.16, 1.75, -0.10),
                        age: 0.0,
                        lifetime: 0.95,
                        gravity: -0.55,
                        start_size: 0.38,
                        end_size: 0.96,
                        color_rgba: [0.16, 0.16, 0.18, 0.65],
                    });
                }
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
        let ent_vis = cached_fow.is_entity_visible(world, loaded, ent);
        if matches!(ent.kind, EntityKind::Building { .. }) {
            let is_done = ent.construction_progress >= 1.0;
            let was_done = tracker
                .known_construction_done
                .insert(ent.id, is_done)
                .unwrap_or(is_done);
            if is_done && !was_done {
                if ent_vis {
                    commands.spawn((
                        BattlefieldVfxEntity,
                        ShockwaveRingVfx {
                            center: Vec3::new(
                                ent.position[0],
                                ent.position[1].max(0.0) + 0.20,
                                ent.position[2],
                            ),
                            max_radius: 4.2,
                            age: 0.0,
                            lifetime: 0.65,
                            color: Color::srgb(0.25, 0.98, 0.48),
                        },
                    ));
                }
                if ent.player_id == pov_pid {
                    interaction.push_eva_alert("EVA: Construction complete!");
                }
            }
        }

        let prev_rank = tracker
            .known_veterancy
            .insert(ent.id, ent.veterancy)
            .unwrap_or(VeterancyRank::Rookie);
        if ent.veterancy != prev_rank && ent.veterancy != VeterancyRank::Rookie {
            if ent_vis {
                let promo_pos = Vec3::new(
                    ent.position[0],
                    ent.position[1].max(0.0) + 0.45,
                    ent.position[2],
                );
                commands.spawn((
                    BattlefieldVfxEntity,
                    ShockwaveRingVfx {
                        center: promo_pos,
                        max_radius: 3.4,
                        age: 0.0,
                        lifetime: 0.65,
                        color: Color::srgb(1.0, 0.88, 0.18),
                    },
                ));
            }
            if ent.player_id == pov_pid {
                interaction.push_eva_alert(format!(
                    "EVA: Unit #{} promoted to {:?} rank!",
                    ent.id, ent.veterancy
                ));
            }
        }
    }

    spawn_combat_and_destruction_events(
        &mut commands,
        loaded,
        world,
        &cached_fow,
        pov_pid,
        assets,
        gpu_pool.as_deref_mut(),
        socket_reg.as_deref_mut(),
        &mut materials,
        &mut interaction,
        &mut tracker,
        &mut camera_q,
    );
}
