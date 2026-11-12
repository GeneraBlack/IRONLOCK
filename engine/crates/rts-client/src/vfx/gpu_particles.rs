use crate::camera::RtsCameraRig;
use crate::rendering::{MapEnvironmentEntity, PreviewStageCamera};
use crate::SimWorldResource;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use rts_simulation::pathfinding::CollisionGrid;
use std::collections::HashMap;

pub const MAX_GPU_PARTICLE_INSTANCES: usize = 4_096;

/// Internal WGSL handle for `assets/shaders/particles_instanced.wgsl` (`Task 238`).
pub const PARTICLES_INSTANCED_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x71e4_b2a9_58c3_4f10_93d1_8a6c_2e4b_9034);

#[allow(dead_code)]
mod shader_uniforms {
    use bevy::prelude::Vec4;
    use bevy::render::render_resource::ShaderType;

    /// Uniform buffer (`@group(2) @binding(0)`) consumed by `particles_instanced.wgsl`.
    #[derive(ShaderType, Debug, Clone, Copy, PartialEq)]
    #[repr(C)]
    pub struct GpuParticleUniform {
        /// `xyz = camera_right`, `w = current_time_secs`
        pub camera_right_and_time: Vec4,
        /// `xyz = camera_up`, `w = floor_bounce_y`
        pub camera_up_and_floor: Vec4,
        /// `rgb = end_tint_rgb`, `w = global_hdr_boost`
        pub end_tint_and_boost: Vec4,
    }
}
pub use shader_uniforms::GpuParticleUniform;

impl Default for GpuParticleUniform {
    fn default() -> Self {
        Self {
            camera_right_and_time: Vec4::new(1.0, 0.0, 0.0, 0.0),
            camera_up_and_floor: Vec4::new(0.0, 1.0, 0.0, 0.08),
            end_tint_and_boost: Vec4::new(0.72, 0.56, 0.44, 1.45),
        }
    }
}

/// GPU vertex+fragment particle billboard material (`particles_instanced.wgsl` — `Task 238`).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GpuParticleShaderMaterial {
    #[uniform(0)]
    pub params: GpuParticleUniform,
    #[texture(1)]
    #[sampler(2)]
    pub particle_tex: Handle<Image>,
    pub alpha_mode: AlphaMode,
}

impl Material for GpuParticleShaderMaterial {
    fn vertex_shader() -> ShaderRef {
        PARTICLES_INSTANCED_SHADER_HANDLE.into()
    }

    fn fragment_shader() -> ShaderRef {
        PARTICLES_INSTANCED_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

/// Marker component for the single GPU-batched dynamic mesh rendering all active
/// `EnergyBeamVfx`, `ShockwaveRingVfx`, `CommandMarkerVfx`, and data-driven beams/ribbons.
#[derive(Component)]
pub struct VfxDynamicBatchEntity;

/// Marker component for the single GPU-batched camera-facing billboard mesh rendering all
/// active `GpuParticleBurstBuffer` particle instances via `particles_instanced.wgsl`.
#[derive(Component)]
pub struct VfxGpuParticleBillboardEntity;

#[derive(Debug, Clone, Copy)]
pub struct GpuParticleInstance {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub lifetime: f32,
    pub gravity: f32,
    pub start_size: f32,
    pub end_size: f32,
    pub color_rgba: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct GpuShaderParticleMeta {
    pub spawn_origin: Vec3,
    pub initial_velocity: Vec3,
    pub spawn_time_secs: Option<f32>,
    pub drag: f32,
}

/// Contiguous particle burst buffer driving `particles_instanced.wgsl` (`spawn_time`, `gravity`,
/// `drag`, size ramp, and HDR color/alpha ramp on the GPU) with a deterministic CPU fallback for
/// headless unit tests.
#[derive(Debug, Clone, Default)]
pub struct GpuParticleBurstBuffer {
    pub instances: Vec<GpuParticleInstance>,
    shader_meta: Vec<GpuShaderParticleMeta>,
    dirty_gpu_upload: bool,
}

impl GpuParticleBurstBuffer {
    pub fn emit(&mut self, particle: GpuParticleInstance) {
        self.emit_with_drag(particle, 0.24);
    }

    pub fn emit_with_drag(&mut self, particle: GpuParticleInstance, drag: f32) {
        if self.shader_meta.len() != self.instances.len() {
            self.rebuild_shader_meta();
        }
        if self.instances.len() >= MAX_GPU_PARTICLE_INSTANCES {
            self.instances.swap_remove(0);
            self.shader_meta.swap_remove(0);
        }
        self.shader_meta.push(GpuShaderParticleMeta {
            spawn_origin: particle.position,
            initial_velocity: particle.velocity,
            spawn_time_secs: None,
            drag: drag.max(0.0),
        });
        self.instances.push(particle);
        self.dirty_gpu_upload = true;
    }

    pub fn clear(&mut self) {
        if !self.instances.is_empty() {
            self.dirty_gpu_upload = true;
        }
        self.instances.clear();
        self.shader_meta.clear();
    }

    fn rebuild_shader_meta(&mut self) {
        self.shader_meta = self
            .instances
            .iter()
            .map(|p| GpuShaderParticleMeta {
                spawn_origin: p.position,
                initial_velocity: p.velocity,
                spawn_time_secs: None,
                drag: 0.24,
            })
            .collect();
        self.dirty_gpu_upload = true;
    }

    /// Uploads static per-particle kinematics (`spawn_origin`, `init_vel`, `spawn_time`, `lifetime`,
    /// `gravity`, `drag`, `start_size`, `end_size`, `color_rgba`) to `mesh` ONLY when bursts arrive
    /// or old particles expire — `particles_instanced.wgsl` evaluates motion/size/color every frame on the GPU!
    pub fn step_and_upload_gpu_shader_particles(
        &mut self,
        now_secs: f32,
        dt: f32,
        mesh: &mut Mesh,
    ) {
        if self.shader_meta.len() != self.instances.len() {
            self.rebuild_shader_meta();
        }

        let mut i = 0usize;
        while i < self.instances.len() {
            if self.shader_meta[i].spawn_time_secs.is_none() {
                self.shader_meta[i].spawn_time_secs = Some(now_secs - self.instances[i].age);
                self.dirty_gpu_upload = true;
            }
            let spawn_t = self.shader_meta[i].spawn_time_secs.unwrap_or(now_secs);
            let age = (now_secs - spawn_t).max(self.instances[i].age + dt);
            self.instances[i].age = age;
            if age >= self.instances[i].lifetime {
                self.instances.swap_remove(i);
                self.shader_meta.swap_remove(i);
                self.dirty_gpu_upload = true;
                continue;
            }
            i += 1;
        }

        if !self.dirty_gpu_upload {
            return;
        }
        self.dirty_gpu_upload = false;

        let count = self.instances.len();
        let mut positions = Vec::with_capacity(count * 4);
        let mut normals = Vec::with_capacity(count * 4);
        let mut uvs_a = Vec::with_capacity(count * 4);
        let mut uvs_b = Vec::with_capacity(count * 4);
        let mut tangents = Vec::with_capacity(count * 4);
        let mut colors = Vec::with_capacity(count * 4);
        let mut indices = Vec::with_capacity(count * 6);

        for (p, meta) in self.instances.iter().zip(self.shader_meta.iter()) {
            let spawn_t = meta.spawn_time_secs.unwrap_or(now_secs);
            let origin = meta.spawn_origin.to_array();
            let vel = meta.initial_velocity.to_array();
            let uvb = [spawn_t, p.lifetime.max(0.02)];
            let kin = [
                p.gravity,
                meta.drag,
                p.start_size.max(0.02),
                p.end_size.max(0.01),
            ];

            let base = positions.len() as u32;
            positions.extend_from_slice(&[origin; 4]);
            normals.extend_from_slice(&[vel; 4]);
            uvs_a.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
            uvs_b.extend_from_slice(&[uvb; 4]);
            tangents.extend_from_slice(&[kin; 4]);
            colors.extend_from_slice(&[p.color_rgba; 4]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs_a);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs_b);
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
    }

    pub fn step_and_build_billboards(
        &mut self,
        dt: f32,
        grid: Option<&CollisionGrid>,
        cam_right: Vec3,
        cam_up: Vec3,
        mesh: &mut Mesh,
    ) {
        if self.shader_meta.len() != self.instances.len() {
            self.rebuild_shader_meta();
        }
        let mut i = 0usize;
        while i < self.instances.len() {
            let drag = self.shader_meta[i].drag;
            let p = &mut self.instances[i];
            p.age += dt;
            if p.age >= p.lifetime {
                self.instances.swap_remove(i);
                self.shader_meta.swap_remove(i);
                continue;
            }
            if drag > 0.001 {
                let damp = (1.0 - drag * dt).clamp(0.25, 1.0);
                p.velocity.x *= damp;
                p.velocity.z *= damp;
            }
            p.velocity.y -= p.gravity * dt;
            p.position += p.velocity * dt;

            if p.gravity > 0.0 {
                let floor_y = grid
                    .map(|g| g.sample_world_height(p.position.x, p.position.z) + 0.06)
                    .unwrap_or(0.08);
                if p.position.y < floor_y {
                    p.position.y = floor_y;
                    p.velocity.y = -p.velocity.y * 0.36;
                    p.velocity.x *= 0.72;
                    p.velocity.z *= 0.72;
                }
            }
            i += 1;
        }

        let count = self.instances.len();
        let mut positions = Vec::with_capacity(count * 4);
        let mut normals = Vec::with_capacity(count * 4);
        let mut uvs = Vec::with_capacity(count * 4);
        let mut colors = Vec::with_capacity(count * 4);
        let mut indices = Vec::with_capacity(count * 6);

        let r_axis = cam_right.normalize_or_zero();
        let u_axis = cam_up.normalize_or_zero();
        let r_axis = if r_axis.length_squared() > 0.01 {
            r_axis
        } else {
            Vec3::X
        };
        let u_axis = if u_axis.length_squared() > 0.01 {
            u_axis
        } else {
            Vec3::Y
        };
        let n_arr = r_axis.cross(u_axis).normalize_or_zero().to_array();

        for p in &self.instances {
            let t = (p.age / p.lifetime.max(0.001)).clamp(0.0, 1.0);
            let half = (p.start_size + (p.end_size - p.start_size) * t).max(0.02) * 0.5;
            let fade = (1.0 - t).powf(0.85);
            let col = [
                p.color_rgba[0],
                p.color_rgba[1],
                p.color_rgba[2],
                (p.color_rgba[3] * fade).clamp(0.0, 1.0),
            ];
            let rx = r_axis * half;
            let uy = u_axis * half;

            let base = positions.len() as u32;
            positions.extend_from_slice(&[
                (p.position - rx - uy).to_array(),
                (p.position + rx - uy).to_array(),
                (p.position + rx + uy).to_array(),
                (p.position - rx + uy).to_array(),
            ]);
            normals.extend_from_slice(&[n_arr; 4]);
            uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
            colors.extend_from_slice(&[col; 4]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        mesh.remove_attribute(Mesh::ATTRIBUTE_UV_1);
        mesh.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
    }
}

/// Synthesizes a soft radial glow sprite (`32x32` RGBA8) so GPU-instanced billboards have smooth
/// circular feathered edges rather than hard square corners.
pub fn build_soft_particle_disc_image() -> Image {
    let size = 32u32;
    let mut bytes = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        let ny = ((y as f32 + 0.5) / size as f32) * 2.0 - 1.0;
        for x in 0..size {
            let nx = ((x as f32 + 0.5) / size as f32) * 2.0 - 1.0;
            let r = (nx * nx + ny * ny).sqrt();
            let alpha = (1.0 - r).clamp(0.0, 1.0).powf(1.65);
            let core = (1.0 - r * 0.45).clamp(0.55, 1.0);
            let c = (core * 255.0).round() as u8;
            let a = (alpha * 255.0).round() as u8;
            bytes.extend_from_slice(&[c, c, c, a]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        bytes,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Shared GPU particle & VFX material/mesh cache (`vfx/gpu_particles.rs` — Tasks 163, 194 & 238).
#[derive(Resource, Default)]
pub struct GpuParticlePoolResources {
    pub initialized: bool,
    pub uses_gpu_shader: bool,
    pub batch_mesh: Handle<Mesh>,
    pub batch_material: Handle<StandardMaterial>,
    pub particle_mesh: Handle<Mesh>,
    pub particle_material: Handle<StandardMaterial>,
    pub particle_shader_material: Handle<GpuParticleShaderMaterial>,
    pub soft_disc_texture: Handle<Image>,
    pub burst_buffer: GpuParticleBurstBuffer,
    pub flipbook_frame_materials: HashMap<(String, u32), Handle<StandardMaterial>>,
    pub cached_sheet_ids: HashMap<String, AssetId<Image>>,
}

impl GpuParticlePoolResources {
    #[allow(clippy::too_many_arguments)]
    pub fn ensure_initialized(
        &mut self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        shader_materials: Option<&mut Assets<GpuParticleShaderMaterial>>,
        images: Option<&mut Assets<Image>>,
        batch_entity_missing: bool,
        particle_entity_missing: bool,
    ) {
        if !self.initialized {
            self.batch_mesh = meshes.add(new_empty_vfx_batch_mesh());
            self.batch_material = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: LinearRgba::new(1.65, 1.65, 1.65, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });
            self.particle_mesh = meshes.add(new_empty_particle_billboard_mesh());
            let soft_tex = images.map(|imgs| imgs.add(build_soft_particle_disc_image()));
            if let Some(ref tex) = soft_tex {
                self.soft_disc_texture = tex.clone();
            }
            self.particle_material = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: soft_tex.clone(),
                emissive: LinearRgba::new(1.35, 1.25, 1.10, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });
            if let (Some(sm), Some(tex)) = (shader_materials, soft_tex) {
                self.particle_shader_material = sm.add(GpuParticleShaderMaterial {
                    params: GpuParticleUniform::default(),
                    particle_tex: tex,
                    alpha_mode: AlphaMode::Blend,
                });
                self.uses_gpu_shader = true;
            }
            self.initialized = true;
        }
        if batch_entity_missing {
            commands.spawn((
                MapEnvironmentEntity,
                VfxDynamicBatchEntity,
                Mesh3d(self.batch_mesh.clone()),
                MeshMaterial3d(self.batch_material.clone()),
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                bevy::render::view::NoFrustumCulling,
                Transform::IDENTITY,
            ));
        }
        if particle_entity_missing {
            if self.uses_gpu_shader {
                commands.spawn((
                    MapEnvironmentEntity,
                    VfxGpuParticleBillboardEntity,
                    Mesh3d(self.particle_mesh.clone()),
                    MeshMaterial3d(self.particle_shader_material.clone()),
                    bevy::pbr::NotShadowCaster,
                    bevy::pbr::NotShadowReceiver,
                    bevy::render::view::NoFrustumCulling,
                    Transform::IDENTITY,
                ));
            } else {
                commands.spawn((
                    MapEnvironmentEntity,
                    VfxGpuParticleBillboardEntity,
                    Mesh3d(self.particle_mesh.clone()),
                    MeshMaterial3d(self.particle_material.clone()),
                    bevy::pbr::NotShadowCaster,
                    bevy::pbr::NotShadowReceiver,
                    bevy::render::view::NoFrustumCulling,
                    Transform::IDENTITY,
                ));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn shared_flipbook_frame_material(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        sheet_key: &str,
        sheet_img: &Handle<Image>,
        frame_count: u32,
        columns: u32,
        rows: u32,
        frame_idx: u32,
    ) -> Handle<StandardMaterial> {
        let total = frame_count.max(1);
        let cols = columns.max(1);
        let rws = rows.max(1);
        let clamped_frame = frame_idx.min(total - 1);

        let img_id = sheet_img.id();
        if self.cached_sheet_ids.get(sheet_key).copied() != Some(img_id) {
            self.flipbook_frame_materials
                .retain(|(k, _), _| k.as_str() != sheet_key);
            self.cached_sheet_ids.insert(sheet_key.to_string(), img_id);
        }

        let lookup_key = (sheet_key.to_string(), clamped_frame);
        if let Some(existing) = self.flipbook_frame_materials.get(&lookup_key) {
            return existing.clone();
        }

        let inv_c = 1.0 / cols as f32;
        let inv_r = 1.0 / rws as f32;
        for f in 0..total {
            let col_idx = f % cols;
            let row_idx = (f / cols) % rws;
            let uv_tf = bevy::math::Affine2::from_scale_angle_translation(
                Vec2::new(inv_c, inv_r),
                0.0,
                Vec2::new(col_idx as f32 * inv_c, row_idx as f32 * inv_r),
            );
            let h = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(sheet_img.clone()),
                emissive: LinearRgba::new(1.8, 1.2, 0.5, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                uv_transform: uv_tf,
                ..default()
            });
            self.flipbook_frame_materials
                .insert((sheet_key.to_string(), f), h);
        }

        self.flipbook_frame_materials
            .get(&lookup_key)
            .cloned()
            .expect("frame material just populated")
    }
}

/// Synchronizes camera billboard axes, elapsed simulation/render time, and local terrain floor
/// height into `GpuParticleShaderMaterial` every frame (`Task 238`).
pub fn sync_gpu_particle_shader_uniforms(
    time: Option<Res<Time>>,
    sim_res: Option<Res<SimWorldResource>>,
    pool: Option<Res<GpuParticlePoolResources>>,
    shader_materials: Option<ResMut<Assets<GpuParticleShaderMaterial>>>,
    camera_q: Query<(&Transform, &RtsCameraRig), Without<PreviewStageCamera>>,
) {
    let (Some(time), Some(pool), Some(mut mats)) = (time, pool, shader_materials) else {
        return;
    };
    if !pool.uses_gpu_shader {
        return;
    }
    let Some(mat) = mats.get_mut(&pool.particle_shader_material) else {
        return;
    };

    let now = time.elapsed_secs();
    let (cam_right, cam_up, focus_xz) = camera_q
        .iter()
        .next()
        .map(|(tf, rig)| (tf.right().as_vec3(), tf.up().as_vec3(), rig.focus))
        .unwrap_or((Vec3::X, Vec3::Y, Vec3::ZERO));
    let floor_y = sim_res
        .as_deref()
        .map(|sr| sr.0.grid.sample_world_height(focus_xz.x, focus_xz.z) + 0.06)
        .unwrap_or(0.08);

    mat.params.camera_right_and_time = Vec4::new(cam_right.x, cam_right.y, cam_right.z, now);
    mat.params.camera_up_and_floor = Vec4::new(cam_up.x, cam_up.y, cam_up.z, floor_y);
}

pub fn new_empty_vfx_batch_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

pub fn new_empty_particle_billboard_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, Vec::<[f32; 4]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

#[derive(Default)]
pub struct VfxBeamAndRingBatchBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl VfxBeamAndRingBatchBuilder {
    #[inline]
    pub fn push_quad(&mut self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, n: Vec3, rgba: [f32; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&[
            p0.to_array(),
            p1.to_array(),
            p2.to_array(),
            p3.to_array(),
        ]);
        let na = n.to_array();
        self.normals.extend_from_slice(&[na, na, na, na]);
        self.colors.extend_from_slice(&[rgba, rgba, rgba, rgba]);
        self.indices
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    pub fn push_3d_beam_segment(
        &mut self,
        a: Vec3,
        b: Vec3,
        cam_forward: Vec3,
        half_width: f32,
        rgba: [f32; 4],
    ) {
        let dir = b - a;
        let len_sq = dir.length_squared();
        if len_sq < 1e-6 {
            return;
        }
        let unit_dir = dir / len_sq.sqrt();
        let mut side_a = unit_dir.cross(cam_forward);
        if side_a.length_squared() < 1e-4 {
            side_a = unit_dir.cross(Vec3::Y);
        }
        if side_a.length_squared() < 1e-4 {
            side_a = Vec3::X;
        }
        side_a = side_a.normalize() * half_width;
        let side_b = unit_dir.cross(side_a).normalize_or_zero() * half_width;

        self.push_quad(
            a - side_a,
            a + side_a,
            b + side_a,
            b - side_a,
            Vec3::Y,
            rgba,
        );
        self.push_quad(
            a - side_b,
            a + side_b,
            b + side_b,
            b - side_b,
            Vec3::Y,
            rgba,
        );
    }

    pub fn push_ground_ring(
        &mut self,
        center: Vec3,
        radius: f32,
        thickness: f32,
        segments: usize,
        rgba: [f32; 4],
    ) {
        let segs = segments.max(12);
        let r_in = (radius - thickness * 0.5).max(0.02);
        let r_out = radius + thickness * 0.5;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let p0 = Vec3::new(center.x + c0 * r_in, center.y, center.z + s0 * r_in);
            let p1 = Vec3::new(center.x + c0 * r_out, center.y, center.z + s0 * r_out);
            let p2 = Vec3::new(center.x + c1 * r_out, center.y, center.z + s1 * r_out);
            let p3 = Vec3::new(center.x + c1 * r_in, center.y, center.z + s1 * r_in);
            self.push_quad(p0, p1, p2, p3, Vec3::Y, rgba);
        }
    }

    pub fn write_into_mesh(&self, mesh: &mut Mesh) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors.clone());
        mesh.insert_indices(Indices::U32(self.indices.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flipbook_frame_materials_are_shared_across_concurrent_particles_without_cloning_assets() {
        let mut materials = Assets::<StandardMaterial>::default();
        let mut pool = GpuParticlePoolResources::default();
        let dummy_img: Handle<Image> = Handle::weak_from_u128(0xABCD_1234);

        let h1 =
            pool.shared_flipbook_frame_material(&mut materials, "explomed", &dummy_img, 8, 4, 2, 0);
        let count_after_first = materials.len();
        assert_eq!(count_after_first, 8);

        for i in 0..100u32 {
            let hf = pool.shared_flipbook_frame_material(
                &mut materials,
                "explomed",
                &dummy_img,
                8,
                4,
                2,
                i % 8,
            );
            if i % 8 == 0 {
                assert_eq!(hf, h1);
            }
        }
        assert_eq!(materials.len(), count_after_first);
    }

    #[test]
    fn gpu_particle_burst_buffer_uploads_shader_attributes_and_bounces_in_cpu_fallback() {
        let mut buf = GpuParticleBurstBuffer::default();
        for i in 0..10 {
            buf.emit_with_drag(
                GpuParticleInstance {
                    position: Vec3::new(i as f32, 0.2, 2.0),
                    velocity: Vec3::new(0.0, -5.0, 0.0),
                    age: 0.0,
                    lifetime: 1.0,
                    gravity: 12.0,
                    start_size: 0.3,
                    end_size: 0.1,
                    color_rgba: [1.0, 0.7, 0.2, 1.0],
                },
                0.35,
            );
        }
        let mut shader_mesh = new_empty_particle_billboard_mesh();
        buf.step_and_upload_gpu_shader_particles(2.5, 0.016, &mut shader_mesh);
        assert_eq!(shader_mesh.count_vertices(), 40);
        assert!(shader_mesh.attribute(Mesh::ATTRIBUTE_UV_1).is_some());
        assert!(shader_mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_some());

        let mut cpu_mesh = new_empty_particle_billboard_mesh();
        let grid = CollisionGrid::new(16, 16, 2.0);
        buf.step_and_build_billboards(0.1, Some(&grid), Vec3::X, Vec3::Y, &mut cpu_mesh);
        assert_eq!(buf.instances.len(), 10);
        assert_eq!(cpu_mesh.count_vertices(), 40);
        assert!(buf.instances[0].velocity.y > 0.0);
    }
}
