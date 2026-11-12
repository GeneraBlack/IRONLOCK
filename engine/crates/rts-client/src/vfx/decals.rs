use super::decal_atlas::*;

use crate::input::ClientInteractionState;
use crate::rendering::{
    BuildingPointLight, CachedFogVisibility, FogOfWarPlaneEntity, MainDirectionalLight,
    MapEnvironmentEntity, PlacementGhostModel, PreviewStageEntity, SimVisualEntity,
};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rts_sdk::schema::common::LocomotorKind;
use rts_sdk::schema::MovementDomain;
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::{EntityKind, TileVisibility};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{Hash, Hasher};

/// Maximum number of fading ground vehicle tread / tire track quads kept in the ring buffer.
pub const MAX_VEHICLE_TRACK_MARKS: usize = 256;
/// Maximum number of data-driven `VfxEffectDef.ground_decal` stamps kept in the ring buffer (`Task 244`).
pub const MAX_VFX_LINKED_DECAL_STAMPS: usize = 192;
const DECAL_SURFACE_OFFSET_Y: f32 = 0.025;

/// Single GPU-batched height-conforming decal mesh entity rendering craters, scorch rings,
/// oil/radiation stains, and fading tank tread / tire tracks across sloped terrain with PBR normal mapping.
#[derive(Component)]
pub struct BattlefieldDecalBatchEntity;

/// Charred, deformed 3D vehicle wreck mesh spawned over `SimBattlefieldScar { is_wreck: true }`.
#[derive(Component, Debug, Clone)]
pub struct ScarWreckVisualEntity {
    pub scar_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct VehicleTrackMark {
    pub center_xz: [f32; 2],
    pub forward_xz: [f32; 2],
    pub half_width: f32,
    pub half_length: f32,
    pub age_secs: f32,
    pub lifetime_secs: f32,
    pub is_tracked: bool,
}

/// Persistent ground decal stamped by a data-driven `VfxEffectDef` (`ground_decal: Option<VfxDecalLinkSpec>`).
#[derive(Debug, Clone, Copy)]
pub struct VfxLinkedDecalStamp {
    pub center_xz: [f32; 2],
    pub radius: f32,
    pub age_secs: f32,
    pub fade_time_secs: f32,
    pub tile: DecalAtlasTile,
}

#[derive(Resource, Default)]
pub struct DecalSystemState {
    pub initialized: bool,
    pub albedo_atlas: Handle<Image>,
    pub normal_atlas: Handle<Image>,
    pub decal_mesh: Handle<Mesh>,
    pub decal_mat: Handle<StandardMaterial>,
    pub wreck_mesh: Handle<Mesh>,
    pub wreck_mat: Handle<StandardMaterial>,
    pub track_ring: VecDeque<VehicleTrackMark>,
    pub vfx_stamp_ring: VecDeque<VfxLinkedDecalStamp>,
    pub last_vehicle_stamp_xz: BTreeMap<u64, [f32; 2]>,
    pub last_tick: u64,
    pub last_decal_mesh_signature: Option<u64>,
    pub mesh_rebuild_count: u64,
}

impl DecalSystemState {
    pub fn push_track_mark(&mut self, mark: VehicleTrackMark) {
        if self.track_ring.len() >= MAX_VEHICLE_TRACK_MARKS {
            self.track_ring.pop_front();
        }
        self.track_ring.push_back(mark);
    }

    pub fn stamp_vfx_decal(
        &mut self,
        center_xz: [f32; 2],
        decal_kind: &str,
        radius: f32,
        fade_time_secs: f32,
    ) {
        if self.vfx_stamp_ring.len() >= MAX_VFX_LINKED_DECAL_STAMPS {
            self.vfx_stamp_ring.pop_front();
        }
        self.vfx_stamp_ring.push_back(VfxLinkedDecalStamp {
            center_xz,
            radius: radius.clamp(0.4, 9.0),
            age_secs: 0.0,
            fade_time_secs: fade_time_secs.max(1.0),
            tile: DecalAtlasTile::from_decal_kind_str(decal_kind),
        });
    }
}

#[derive(Default)]
pub struct HeightConformingDecalBuilder {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl HeightConformingDecalBuilder {
    fn sample_height_normal_and_tangent(
        grid: &CollisionGrid,
        wx: f32,
        wz: f32,
        tangent_dir_xz: Vec2,
    ) -> (f32, [f32; 3], [f32; 4]) {
        let h = grid.sample_world_height(wx, wz) + DECAL_SURFACE_OFFSET_Y;
        let eps = 0.35;
        let hl = grid.sample_world_height(wx - eps, wz);
        let hr = grid.sample_world_height(wx + eps, wz);
        let hu = grid.sample_world_height(wx, wz - eps);
        let hd = grid.sample_world_height(wx, wz + eps);
        let n = Vec3::new(hl - hr, 2.0 * eps, hu - hd).normalize_or_zero();
        let raw_t = Vec3::new(tangent_dir_xz.x, 0.0, tangent_dir_xz.y);
        let t = (raw_t - n * raw_t.dot(n)).normalize_or_zero();
        let t_safe = if t.length_squared() > 1e-4 {
            t
        } else {
            Vec3::X
        };
        (h, n.to_array(), [t_safe.x, t_safe.y, t_safe.z, 1.0])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_radial_height_decal(
        &mut self,
        grid: &CollisionGrid,
        center_xz: [f32; 2],
        radius: f32,
        subdivs: usize,
        inner_rgba: [f32; 4],
        rim_rgba: [f32; 4],
    ) {
        self.push_radial_height_decal_tiled(
            grid,
            center_xz,
            radius,
            subdivs,
            inner_rgba,
            rim_rgba,
            DecalAtlasTile::Crater,
        );
    }

    /// Appends a subdivided `(subdivs x subdivs)` radial decal grid conforming to `grid.sample_world_height`
    /// with cached `2x2` Albedo+Normal atlas UVs and tangent vectors (`Task 238`).
    #[allow(clippy::too_many_arguments)]
    pub fn push_radial_height_decal_tiled(
        &mut self,
        grid: &CollisionGrid,
        center_xz: [f32; 2],
        radius: f32,
        subdivs: usize,
        inner_rgba: [f32; 4],
        rim_rgba: [f32; 4],
        tile: DecalAtlasTile,
    ) {
        let sub = subdivs.max(2);
        let vw = sub + 1;
        let r = radius.max(0.25);
        let base = self.positions.len() as u32;

        for iz in 0..=sub {
            let v_frac = iz as f32 / sub as f32;
            let nz = v_frac * 2.0 - 1.0;
            for ix in 0..=sub {
                let u_frac = ix as f32 / sub as f32;
                let nx = u_frac * 2.0 - 1.0;
                let wx = center_xz[0] + nx * r;
                let wz = center_xz[1] + nz * r;
                let (wy, n_arr, t_arr) =
                    Self::sample_height_normal_and_tangent(grid, wx, wz, Vec2::X);
                let dist = (nx * nx + nz * nz).sqrt();

                let col = if dist >= 1.0 {
                    [rim_rgba[0], rim_rgba[1], rim_rgba[2], 0.0]
                } else if dist < 0.55 {
                    let t = dist / 0.55;
                    [
                        inner_rgba[0] + (rim_rgba[0] - inner_rgba[0]) * t,
                        inner_rgba[1] + (rim_rgba[1] - inner_rgba[1]) * t,
                        inner_rgba[2] + (rim_rgba[2] - inner_rgba[2]) * t,
                        inner_rgba[3] + (rim_rgba[3] - inner_rgba[3]) * t,
                    ]
                } else {
                    let fade = 1.0 - (dist - 0.55) / 0.45;
                    [rim_rgba[0], rim_rgba[1], rim_rgba[2], rim_rgba[3] * fade]
                };

                self.positions.push([wx, wy, wz]);
                self.normals.push(n_arr);
                self.uvs.push(tile.map_local_uv(u_frac, v_frac));
                self.tangents.push(t_arr);
                self.colors.push(col);
            }
        }

        for qz in 0..sub {
            for qx in 0..sub {
                let i00 = base + (qz * vw + qx) as u32;
                let i10 = i00 + 1;
                let i01 = base + ((qz + 1) * vw + qx) as u32;
                let i11 = i01 + 1;
                self.indices
                    .extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
            }
        }
    }

    /// Appends an oriented height-conforming quad for a single tank tread or tire track imprint.
    pub fn push_oriented_track_quad(
        &mut self,
        grid: &CollisionGrid,
        mark: &VehicleTrackMark,
        alpha_scale: f32,
    ) {
        let fwd = Vec2::new(mark.forward_xz[0], mark.forward_xz[1]).normalize_or_zero();
        let fwd = if fwd.length_squared() < 0.01 {
            Vec2::Y
        } else {
            fwd
        };
        let right = Vec2::new(fwd.y, -fwd.x);
        let c = Vec2::new(mark.center_xz[0], mark.center_xz[1]);

        let corners = [
            (
                c - right * mark.half_width - fwd * mark.half_length,
                [0.0, 0.0],
            ),
            (
                c + right * mark.half_width - fwd * mark.half_length,
                [1.0, 0.0],
            ),
            (
                c + right * mark.half_width + fwd * mark.half_length,
                [1.0, 1.0],
            ),
            (
                c - right * mark.half_width + fwd * mark.half_length,
                [0.0, 1.0],
            ),
        ];
        let col = if mark.is_tracked {
            [0.20, 0.17, 0.14, 0.72 * alpha_scale]
        } else {
            [0.24, 0.21, 0.18, 0.56 * alpha_scale]
        };

        let base = self.positions.len() as u32;
        for (pt, luv) in corners {
            let (wy, n_arr, t_arr) =
                Self::sample_height_normal_and_tangent(grid, pt.x, pt.y, right);
            self.positions.push([pt.x, wy, pt.y]);
            self.normals.push(n_arr);
            self.uvs
                .push(DecalAtlasTile::TankTireTrack.map_local_uv(luv[0], luv[1]));
            self.tangents.push(t_arr);
            self.colors.push(col);
        }
        self.indices
            .extend_from_slice(&[base, base + 3, base + 1, base + 1, base + 3, base + 2]);
    }

    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, self.tangents);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }

    pub fn write_into_mesh(&mut self, mesh: &mut Mesh) {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            std::mem::take(&mut self.positions),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, std::mem::take(&mut self.normals));
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, std::mem::take(&mut self.uvs));
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, std::mem::take(&mut self.tangents));
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut self.colors));
        mesh.insert_indices(Indices::U32(std::mem::take(&mut self.indices)));
    }
}

#[inline]
fn quantize_fade(f: f32) -> u8 {
    (f.clamp(0.0, 1.0) * 120.0).round() as u8
}

/// Synchronizes height-conforming ground decals (craters, scorch rings, oil/radiation stains, fading
/// tank tread / tire tracks) and charred 3D vehicle wrecks every frame, performing `0` CPU mesh
/// rebuilds when the visible decal signature is unchanged (`Task 238`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_battlefield_decals_and_tracks(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    interaction: Option<Res<ClientInteractionState>>,
    cached_fow: Option<Res<CachedFogVisibility>>,
    mut state: ResMut<DecalSystemState>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    mut images: Option<ResMut<Assets<Image>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    batch_q: Query<Entity, With<BattlefieldDecalBatchEntity>>,
    mut wreck_q: Query<
        (
            Entity,
            &ScarWreckVisualEntity,
            &mut Transform,
            &mut Visibility,
        ),
        (
            Without<PreviewStageEntity>,
            Without<SimVisualEntity>,
            Without<BuildingPointLight>,
            Without<crate::camera::RtsCameraRig>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<MainDirectionalLight>,
        ),
    >,
) {
    let (Some(mod_res), Some(sim_res), Some(cached_fow), Some(mut meshes), Some(mut materials)) =
        (mod_res, sim_res, cached_fow, meshes, materials)
    else {
        return;
    };
    let _ = interaction;
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let dt = time.delta_secs();

    if !state.initialized {
        state.initialized = true;
        let (albedo_opt, normal_opt) = if let Some(imgs) = images.as_deref_mut() {
            let (alb, nrm) = build_procedural_decal_albedo_and_normal_atlases();
            let ha = imgs.add(alb);
            let hn = imgs.add(nrm);
            state.albedo_atlas = ha.clone();
            state.normal_atlas = hn.clone();
            (Some(ha), Some(hn))
        } else {
            (None, None)
        };
        state.decal_mesh = meshes.add(HeightConformingDecalBuilder::default().into_mesh());
        state.decal_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: albedo_opt,
            normal_map_texture: normal_opt,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.90,
            metallic: 0.04,
            unlit: false,
            cull_mode: None,
            ..default()
        });
        state.wreck_mesh = meshes.add(build_charred_vehicle_wreck_mesh());
        state.wreck_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.88,
            metallic: 0.35,
            ..default()
        });
    }

    if batch_q.is_empty() {
        commands.spawn((
            MapEnvironmentEntity,
            BattlefieldDecalBatchEntity,
            Mesh3d(state.decal_mesh.clone()),
            MeshMaterial3d(state.decal_mat.clone()),
            bevy::pbr::NotShadowCaster,
            bevy::render::view::NoFrustumCulling,
            Transform::IDENTITY,
        ));
    }

    if world.tick < state.last_tick {
        state.track_ring.clear();
        state.vfx_stamp_ring.clear();
        state.last_vehicle_stamp_xz.clear();
        state.last_decal_mesh_signature = None;
    }
    state.last_tick = world.tick;

    // 1. Age & prune fading tank tread / tire track imprints + VFX-linked decals
    for mark in &mut state.track_ring {
        mark.age_secs += dt;
    }
    while state
        .track_ring
        .front()
        .is_some_and(|m| m.age_secs >= m.lifetime_secs)
    {
        state.track_ring.pop_front();
    }
    for stamp in &mut state.vfx_stamp_ring {
        stamp.age_secs += dt;
    }
    while state
        .vfx_stamp_ring
        .front()
        .is_some_and(|s| s.age_secs >= s.fade_time_secs)
    {
        state.vfx_stamp_ring.pop_front();
    }

    // 2. Emit parallel left/right tread or tire tracks behind moving visible ground vehicles
    let mut alive_vehicle_ids = std::collections::BTreeSet::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || ent.burrowed
            || !cached_fow.is_entity_visible(world, loaded, ent)
        {
            continue;
        }
        let EntityKind::Unit { unit_id } = &ent.kind else {
            continue;
        };
        let Some(udef) = loaded.units.get(unit_id) else {
            continue;
        };
        if udef.movement_domain != MovementDomain::Ground {
            continue;
        }
        let is_tracked = match udef.locomotor.kind {
            Some(LocomotorKind::Tracked) => true,
            Some(LocomotorKind::Wheeled) => false,
            Some(_) => continue,
            None => {
                if ent.max_health < 180.0 {
                    continue;
                }
                true
            }
        };

        alive_vehicle_ids.insert(ent.id);
        let curr_xz = [ent.position[0], ent.position[2]];
        let prev_xz = state.last_vehicle_stamp_xz.entry(ent.id).or_insert(curr_xz);
        let dx = curr_xz[0] - prev_xz[0];
        let dz = curr_xz[1] - prev_xz[1];
        let dist_sq = dx * dx + dz * dz;
        if dist_sq >= 0.85 * 0.85 {
            let dist = dist_sq.sqrt();
            let fwd = [dx / dist, dz / dist];
            let right = [fwd[1], -fwd[0]];
            let axle_half = if is_tracked { 0.48 } else { 0.40 };
            let mid_xz = [
                (prev_xz[0] + curr_xz[0]) * 0.5,
                (prev_xz[1] + curr_xz[1]) * 0.5,
            ];
            *prev_xz = curr_xz;

            for side in [-1.0_f32, 1.0_f32] {
                state.push_track_mark(VehicleTrackMark {
                    center_xz: [
                        mid_xz[0] + right[0] * axle_half * side,
                        mid_xz[1] + right[1] * axle_half * side,
                    ],
                    forward_xz: fwd,
                    half_width: if is_tracked { 0.16 } else { 0.10 },
                    half_length: 0.46,
                    age_secs: 0.0,
                    lifetime_secs: 14.0,
                    is_tracked,
                });
            }
        }
    }
    state
        .last_vehicle_stamp_xz
        .retain(|id, _| alive_vehicle_ids.contains(id));

    // 3. Compute dirty-state signature so idle frames perform ZERO CPU mesh rebuilds (`Task 238`)
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for mark in &state.track_ring {
        let pos3 = [mark.center_xz[0], 0.0, mark.center_xz[1]];
        if cached_fow.world_pos_visibility(world, pos3) == TileVisibility::Unexplored {
            continue;
        }
        mark.center_xz[0].to_bits().hash(&mut hasher);
        mark.center_xz[1].to_bits().hash(&mut hasher);
        quantize_fade(1.0 - mark.age_secs / mark.lifetime_secs.max(0.01)).hash(&mut hasher);
    }
    for stamp in &state.vfx_stamp_ring {
        let pos3 = [stamp.center_xz[0], 0.0, stamp.center_xz[1]];
        if cached_fow.world_pos_visibility(world, pos3) == TileVisibility::Unexplored {
            continue;
        }
        stamp.center_xz[0].to_bits().hash(&mut hasher);
        stamp.center_xz[1].to_bits().hash(&mut hasher);
        stamp.tile.hash(&mut hasher);
        quantize_fade(1.0 - stamp.age_secs / stamp.fade_time_secs.max(0.01)).hash(&mut hasher);
    }
    for scar in &world.battlefield_scars {
        if cached_fow.world_pos_visibility(world, scar.position) == TileVisibility::Unexplored {
            continue;
        }
        scar.position[0].to_bits().hash(&mut hasher);
        scar.position[2].to_bits().hash(&mut hasher);
        quantize_fade(scar.remaining_secs / 8.0).hash(&mut hasher);
    }
    for rad in &world.radiation_zones {
        if cached_fow.world_pos_visibility(world, rad.position) == TileVisibility::Unexplored {
            continue;
        }
        rad.position[0].to_bits().hash(&mut hasher);
        rad.position[2].to_bits().hash(&mut hasher);
        quantize_fade(rad.remaining_secs / 3.0).hash(&mut hasher);
    }
    let sig = hasher.finish();

    if state.last_decal_mesh_signature != Some(sig) {
        state.last_decal_mesh_signature = Some(sig);
        state.mesh_rebuild_count += 1;

        let mut builder = HeightConformingDecalBuilder::default();
        for mark in &state.track_ring {
            let pos3 = [mark.center_xz[0], 0.0, mark.center_xz[1]];
            if cached_fow.world_pos_visibility(world, pos3) == TileVisibility::Unexplored {
                continue;
            }
            let fade = (1.0 - mark.age_secs / mark.lifetime_secs.max(0.01)).clamp(0.0, 1.0);
            builder.push_oriented_track_quad(&world.grid, mark, fade);
        }
        for stamp in &state.vfx_stamp_ring {
            let pos3 = [stamp.center_xz[0], 0.0, stamp.center_xz[1]];
            if cached_fow.world_pos_visibility(world, pos3) == TileVisibility::Unexplored {
                continue;
            }
            let fade = (1.0 - stamp.age_secs / stamp.fade_time_secs.max(0.01)).clamp(0.0, 1.0);
            let (inner, rim) = match stamp.tile {
                DecalAtlasTile::OilRadiationStain => (
                    [0.14, 0.13, 0.11, 0.80 * fade],
                    [0.24, 0.22, 0.18, 0.45 * fade],
                ),
                DecalAtlasTile::Scorch => (
                    [0.10, 0.09, 0.08, 0.88 * fade],
                    [0.24, 0.18, 0.14, 0.50 * fade],
                ),
                _ => (
                    [0.12, 0.10, 0.09, 0.92 * fade],
                    [0.32, 0.26, 0.20, 0.62 * fade],
                ),
            };
            builder.push_radial_height_decal_tiled(
                &world.grid,
                stamp.center_xz,
                stamp.radius,
                4,
                inner,
                rim,
                stamp.tile,
            );
        }
        for scar in &world.battlefield_scars {
            if cached_fow.world_pos_visibility(world, scar.position) == TileVisibility::Unexplored {
                continue;
            }
            let fade = (scar.remaining_secs / 8.0).clamp(0.08, 1.0);
            let r = scar.radius.clamp(0.95, 4.5);
            let tile = if scar.is_wreck {
                DecalAtlasTile::Scorch
            } else {
                DecalAtlasTile::Crater
            };
            builder.push_radial_height_decal_tiled(
                &world.grid,
                [scar.position[0], scar.position[2]],
                r,
                4,
                [0.12, 0.10, 0.09, 0.92 * fade],
                [0.30, 0.24, 0.18, 0.60 * fade],
                tile,
            );
        }
        for rad in &world.radiation_zones {
            if cached_fow.world_pos_visibility(world, rad.position) == TileVisibility::Unexplored {
                continue;
            }
            let fade = (rad.remaining_secs / 3.0).clamp(0.15, 1.0);
            let r = rad.radius.clamp(1.2, 8.0);
            let (inner, rim) = if rad.is_toxic_cloud {
                (
                    [0.26, 0.78, 0.18, 0.56 * fade],
                    [0.16, 0.52, 0.12, 0.30 * fade],
                )
            } else {
                (
                    [0.20, 0.96, 0.16, 0.62 * fade],
                    [0.12, 0.60, 0.10, 0.34 * fade],
                )
            };
            builder.push_radial_height_decal_tiled(
                &world.grid,
                [rad.position[0], rad.position[2]],
                r,
                4,
                inner,
                rim,
                DecalAtlasTile::OilRadiationStain,
            );
        }

        if let Some(m) = meshes.get_mut(&state.decal_mesh) {
            builder.write_into_mesh(m);
        }
    }

    // 4. Sync charred 3D vehicle wreck hulks over `world.battlefield_scars` (`is_wreck == true`)
    let wreck_scars: Vec<(usize, &rts_simulation::world::SimBattlefieldScar)> = world
        .battlefield_scars
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_wreck && s.remaining_secs > 0.05)
        .collect();

    let desired_count = wreck_scars.len();
    let mut seen_slots = 0usize;
    for (ent, wreck, mut tf, mut vis) in &mut wreck_q {
        if wreck.scar_index >= desired_count {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        seen_slots = seen_slots.max(wreck.scar_index + 1);
        let (_, scar) = wreck_scars[wreck.scar_index];
        let visible =
            cached_fow.world_pos_visibility(world, scar.position) != TileVisibility::Unexplored;
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let ground_y = world
            .grid
            .sample_world_height(scar.position[0], scar.position[2]);
        let sink = if scar.remaining_secs < 4.0 {
            (1.0 - scar.remaining_secs / 4.0) * 0.48
        } else {
            0.0
        };
        let yaw =
            (scar.position[0] * 1.7 + scar.position[2] * 2.3).rem_euclid(std::f32::consts::TAU);
        let scale = (scar.radius * 0.72).clamp(0.75, 1.55);
        tf.translation = Vec3::new(scar.position[0], ground_y - sink, scar.position[2]);
        tf.rotation = Quat::from_rotation_y(yaw);
        tf.scale = Vec3::splat(scale);
    }

    if seen_slots < desired_count {
        for (slot, &(_, scar)) in wreck_scars
            .iter()
            .enumerate()
            .take(desired_count)
            .skip(seen_slots)
        {
            let ground_y = world
                .grid
                .sample_world_height(scar.position[0], scar.position[2]);
            let yaw =
                (scar.position[0] * 1.7 + scar.position[2] * 2.3).rem_euclid(std::f32::consts::TAU);
            let scale = (scar.radius * 0.72).clamp(0.75, 1.55);
            let visible =
                cached_fow.world_pos_visibility(world, scar.position) != TileVisibility::Unexplored;
            commands.spawn((
                MapEnvironmentEntity,
                ScarWreckVisualEntity { scar_index: slot },
                Mesh3d(state.wreck_mesh.clone()),
                MeshMaterial3d(state.wreck_mat.clone()),
                Transform::from_xyz(scar.position[0], ground_y, scar.position[2])
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::splat(scale)),
                if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sdk::schema::MapDef;

    #[test]
    fn height_conforming_decal_follows_sloped_terrain_with_normal_map_tangents() {
        let mut grid = CollisionGrid::new(8, 8, 2.0);
        let mut map = MapDef {
            id: "slope_test".to_string(),
            name: "Slope".to_string(),
            width: 8,
            height: 8,
            tile_size: 2.0,
            ..Default::default()
        };
        let mut heights = Vec::with_capacity(9 * 9);
        for _vz in 0..9 {
            for vx in 0..9 {
                heights.push(vx as f32 * 0.5);
            }
        }
        map.heightmap = Some(rts_sdk::schema::HeightmapDef {
            width: 9,
            height: 9,
            heights,
        });
        grid.sync_heightmap_from_map_def(&map);

        let mut builder = HeightConformingDecalBuilder::default();
        builder.push_radial_height_decal(
            &grid,
            [6.0, 6.0],
            2.0,
            4,
            [0.1, 0.1, 0.1, 0.9],
            [0.2, 0.2, 0.2, 0.5],
        );
        assert_eq!(builder.positions.len(), 25);
        assert_eq!(builder.uvs.len(), 25);
        assert_eq!(builder.tangents.len(), 25);
        assert!(builder.positions[4][1] - builder.positions[0][1] > 0.85);

        let (alb, nrm) = build_procedural_decal_albedo_and_normal_atlases();
        assert_eq!(alb.width(), DECAL_ATLAS_SIZE);
        assert_eq!(nrm.width(), DECAL_ATLAS_SIZE);
    }

    #[test]
    fn vehicle_track_ring_buffer_caps_at_256_entries() {
        let mut st = DecalSystemState::default();
        for i in 0..320 {
            st.push_track_mark(VehicleTrackMark {
                center_xz: [i as f32, 0.0],
                forward_xz: [0.0, 1.0],
                half_width: 0.15,
                half_length: 0.45,
                age_secs: 0.0,
                lifetime_secs: 12.0,
                is_tracked: true,
            });
        }
        assert_eq!(st.track_ring.len(), MAX_VEHICLE_TRACK_MARKS);
        assert!((st.track_ring.front().unwrap().center_xz[0] - 64.0).abs() < 1e-5);
    }
}
