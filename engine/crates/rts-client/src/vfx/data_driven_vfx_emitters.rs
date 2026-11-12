use super::{
    DecalSystemState, GpuParticleBurstBuffer, GpuParticleInstance, VfxBeamAndRingBatchBuilder,
};
use crate::camera::RtsCameraRig;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ProjectileStyle, VfxEffectDef, VfxEmitterDef, VfxEmitterShape};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MAX_ACTIVE_DATA_DRIVEN_VFX: usize = 256;
const MAX_RIBBON_POINTS: usize = 12;

/// Dedicated GPU-batched beam/ribbon/ring mesh entity for battlefield `VfxEffectDef` emitters (`Task 244`).
#[derive(Component)]
pub struct DataDrivenVfxBeamBatchEntity;

/// Studio 3D Preview Stage (`RenderLayers::layer(3)`) beam/ribbon/ring batch entity (`Task 244`).
#[derive(Component)]
pub struct PreviewStageVfxBatchEntity;

/// Studio 3D Preview Stage (`RenderLayers::layer(3)`) billboard particle entity (`Task 244`).
#[derive(Component)]
pub struct PreviewStageVfxParticleEntity;

#[derive(Debug, Clone)]
pub struct ActiveEmitterRuntime {
    pub def: VfxEmitterDef,
    pub age_secs: f32,
    pub burst_emitted: bool,
    pub continuous_accum: f32,
    pub ribbon_trail: VecDeque<Vec3>,
}

impl ActiveEmitterRuntime {
    pub fn from_def(def: VfxEmitterDef) -> Self {
        Self {
            def,
            age_secs: 0.0,
            burst_emitted: false,
            continuous_accum: 0.0,
            ribbon_trail: VecDeque::with_capacity(MAX_RIBBON_POINTS),
        }
    }

    pub fn reset_cycle(&mut self) {
        self.age_secs = 0.0;
        self.burst_emitted = false;
        self.continuous_accum = 0.0;
    }
}

#[derive(Debug, Clone)]
pub struct ActiveDataDrivenVfxInstance {
    pub effect_id: String,
    pub origin: Vec3,
    pub target_pos: Vec3,
    pub forward: Vec3,
    pub age_secs: f32,
    pub duration_secs: f32,
    pub looping: bool,
    pub attached_entity_id: Option<u64>,
    pub attached_projectile_id: Option<u64>,
    pub seed: u64,
    pub emitters: Vec<ActiveEmitterRuntime>,
}

/// Runtime manager driving multi-emitter `VfxEffectDef` instances from `LoadedMod.vfx_effects`
/// (`vfx/vfx.toml`), backwards-compatible `ProjectileStyle` fallbacks, camera shake, linked ground
/// decals, and live Studio 3D Preview Stage playback (`Task 244`).
#[derive(Resource, Default)]
pub struct DataDrivenVfxRuntimeState {
    pub initialized: bool,
    pub beam_batch_mesh: Handle<Mesh>,
    pub beam_batch_material: Handle<StandardMaterial>,
    pub preview_beam_mesh: Handle<Mesh>,
    pub preview_particle_mesh: Handle<Mesh>,
    pub preview_particle_material: Handle<StandardMaterial>,
    pub active_instances: Vec<ActiveDataDrivenVfxInstance>,
    pub preview_instance: Option<ActiveDataDrivenVfxInstance>,
    pub preview_burst_buffer: GpuParticleBurstBuffer,
    pub last_sim_tick: u64,
    pub known_entity_death_vfx: BTreeMap<u64, String>,
    pub known_ability_cooldowns: BTreeMap<u64, Vec<f32>>,
    pub active_ambient_entities: BTreeSet<u64>,
    pub active_projectile_vfx: BTreeSet<u64>,
    pub last_preview_signature: u64,
    pub last_paused_scrub_secs: Option<f32>,
    pub spawn_seq: u64,
}

impl DataDrivenVfxRuntimeState {
    pub fn reset_battlefield(&mut self) {
        self.active_instances.clear();
        self.last_sim_tick = 0;
        self.known_entity_death_vfx.clear();
        self.known_ability_cooldowns.clear();
        self.active_ambient_entities.clear();
        self.active_projectile_vfx.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_effect(
        &mut self,
        def: &VfxEffectDef,
        origin: Vec3,
        target_pos: Vec3,
        attached_entity_id: Option<u64>,
        attached_projectile_id: Option<u64>,
        decal_state: Option<&mut DecalSystemState>,
        camera_rig: Option<&mut RtsCameraRig>,
    ) {
        if self.active_instances.len() >= MAX_ACTIVE_DATA_DRIVEN_VFX {
            if let Some(pos) = self.active_instances.iter().position(|i| !i.looping) {
                self.active_instances.remove(pos);
            } else {
                self.active_instances.remove(0);
            }
        }

        if let (Some(spec), Some(decals)) = (&def.ground_decal, decal_state) {
            decals.stamp_vfx_decal(
                [origin.x, origin.z],
                &spec.decal_kind,
                spec.radius,
                spec.fade_time_secs,
            );
        }

        if def.camera_shake_intensity > 0.001 {
            if let Some(rig) = camera_rig {
                let radius = def.camera_shake_radius.max(8.0);
                let dist = Vec2::new(origin.x - rig.focus.x, origin.z - rig.focus.z).length();
                let atten = (1.0 - dist / radius).clamp(0.0, 1.0);
                if atten > 0.01 {
                    rig.add_trauma(origin, def.camera_shake_intensity * atten);
                }
            }
        }

        self.spawn_seq = self.spawn_seq.wrapping_add(1);
        let raw_dir = target_pos - origin;
        let forward = if raw_dir.length_squared() > 1e-4 {
            raw_dir.normalize()
        } else {
            Vec3::Y
        };

        self.active_instances.push(ActiveDataDrivenVfxInstance {
            effect_id: def.id.clone(),
            origin,
            target_pos,
            forward,
            age_secs: 0.0,
            duration_secs: def.duration_secs.max(0.08),
            looping: def.looping,
            attached_entity_id,
            attached_projectile_id,
            seed: self.spawn_seq.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            emitters: def
                .emitters
                .iter()
                .cloned()
                .map(ActiveEmitterRuntime::from_def)
                .collect(),
        });
    }
}

/// Synthesizes a backwards-compatible `VfxEffectDef` for `ProjectileStyle` when a mod omits an explicit `vfx_id`.
pub fn synthesize_fallback_vfx_effect(style: ProjectileStyle) -> VfxEffectDef {
    let (id, ramp, shape, burst) = match style {
        ProjectileStyle::Laser => (
            "fallback_laser",
            [
                [0.45, 1.85, 2.40, 1.0],
                [0.25, 0.95, 1.60, 0.72],
                [0.10, 0.25, 0.45, 0.0],
            ],
            VfxEmitterShape::Beam,
            8,
        ),
        ProjectileStyle::Tesla => (
            "fallback_tesla",
            [
                [0.75, 1.95, 2.80, 1.0],
                [0.35, 1.15, 2.20, 0.80],
                [0.12, 0.35, 0.85, 0.0],
            ],
            VfxEmitterShape::Beam,
            12,
        ),
        ProjectileStyle::Radiation => (
            "fallback_radiation",
            [
                [0.35, 2.40, 0.28, 1.0],
                [0.20, 1.35, 0.16, 0.72],
                [0.08, 0.35, 0.06, 0.0],
            ],
            VfxEmitterShape::Ring,
            14,
        ),
        ProjectileStyle::Sonic => (
            "fallback_sonic",
            [
                [0.85, 2.10, 2.50, 0.95],
                [0.45, 1.20, 1.75, 0.65],
                [0.18, 0.40, 0.60, 0.0],
            ],
            VfxEmitterShape::Ring,
            10,
        ),
        ProjectileStyle::MindControl => (
            "fallback_mind_control",
            [
                [2.10, 0.45, 2.50, 1.0],
                [1.25, 0.22, 1.65, 0.75],
                [0.35, 0.08, 0.55, 0.0],
            ],
            VfxEmitterShape::Beam,
            10,
        ),
        ProjectileStyle::Missile => (
            "fallback_missile",
            [
                [2.50, 1.45, 0.42, 1.0],
                [1.30, 0.52, 0.14, 0.75],
                [0.22, 0.22, 0.24, 0.0],
            ],
            VfxEmitterShape::Cone,
            14,
        ),
        ProjectileStyle::Bullet => (
            "fallback_bullet",
            [
                [2.20, 1.55, 0.50, 1.0],
                [1.15, 0.55, 0.16, 0.68],
                [0.20, 0.18, 0.16, 0.0],
            ],
            VfxEmitterShape::Cone,
            6,
        ),
    };

    VfxEffectDef {
        id: id.to_string(),
        name: id.to_string(),
        duration_secs: 0.55,
        looping: false,
        camera_shake_intensity: 0.0,
        camera_shake_radius: 18.0,
        ground_decal: None,
        fallback_style: style,
        emitters: vec![VfxEmitterDef {
            id: "primary".to_string(),
            shape,
            burst_count: burst,
            duration_secs: 0.45,
            beam_width: 0.18,
            ribbon_width: 0.16,
            color_ramp_rgba: ramp,
            ..Default::default()
        }],
    }
}

pub fn resolve_vfx_effect_def<'a>(
    loaded: &'a LoadedMod,
    effect_id: &str,
) -> Option<&'a VfxEffectDef> {
    loaded.vfx_effects.get(effect_id).or_else(|| {
        let low = effect_id.trim().to_ascii_lowercase();
        loaded
            .vfx_effects
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == low)
            .map(|(_, v)| v)
    })
}

#[inline]
fn det_rand01(seed: u64, salt: u64) -> f32 {
    let mut x = seed
        .wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 30)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((x ^ (x >> 31)) & 0xFFFF) as f32 / 65535.0
}

#[inline]
fn sample_color_ramp(ramp: &[[f32; 4]; 3], t: f32) -> [f32; 4] {
    let tc = t.clamp(0.0, 1.0);
    let (a, b, f) = if tc < 0.5 {
        (&ramp[0], &ramp[1], tc * 2.0)
    } else {
        (&ramp[1], &ramp[2], (tc - 0.5) * 2.0)
    };
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
        (a[3] + (b[3] - a[3]) * f).clamp(0.0, 1.0),
    ]
}

fn emit_particle_from_emitter(
    em: &VfxEmitterDef,
    world_origin: Vec3,
    forward: Vec3,
    seed: u64,
    idx: u64,
    burst_buf: &mut GpuParticleBurstBuffer,
) {
    let r0 = det_rand01(seed, idx * 3 + 1);
    let r1 = det_rand01(seed, idx * 3 + 2);
    let r2 = det_rand01(seed, idx * 3 + 3);
    let l_min = em.lifetime_range_secs[0].max(0.05);
    let l_max = em.lifetime_range_secs[1].max(l_min);
    let lifetime = l_min + (l_max - l_min) * r0;
    let s_min = em.speed_range[0].max(0.0);
    let s_max = em.speed_range[1].max(s_min);
    let speed = s_min + (s_max - s_min) * r1;

    let theta = r2 * std::f32::consts::TAU;
    let phi = (r0 * 2.0 - 1.0).clamp(-1.0, 1.0);
    let ring_r = (1.0 - phi * phi).max(0.0).sqrt();
    let unit_sphere = Vec3::new(
        ring_r * theta.cos(),
        phi.abs() * 0.8 + 0.2,
        ring_r * theta.sin(),
    )
    .normalize_or_zero();

    let (pos_offset, dir) = match em.shape {
        VfxEmitterShape::Point => (Vec3::ZERO, unit_sphere),
        VfxEmitterShape::Sphere => (unit_sphere * (em.sphere_radius.max(0.02) * r1), unit_sphere),
        VfxEmitterShape::Cone => {
            let half_rad = (em.cone_angle_deg.clamp(1.0, 89.0) * 0.5).to_radians();
            let spread = half_rad.tan() * r1;
            let fwd = forward.normalize_or_zero();
            let fwd = if fwd.length_squared() > 0.01 {
                fwd
            } else {
                Vec3::Y
            };
            let up_ref = if fwd.y.abs() > 0.92 { Vec3::X } else { Vec3::Y };
            let right = fwd.cross(up_ref).normalize_or_zero();
            let up = right.cross(fwd).normalize_or_zero();
            let cone_dir =
                (fwd + (right * theta.cos() + up * theta.sin()) * spread).normalize_or_zero();
            (Vec3::ZERO, cone_dir)
        }
        VfxEmitterShape::Ring => {
            let rad = em.sphere_radius.max(0.25);
            let radial = Vec3::new(theta.cos(), 0.15, theta.sin()).normalize_or_zero();
            (
                Vec3::new(theta.cos() * rad, 0.05, theta.sin() * rad),
                radial,
            )
        }
        VfxEmitterShape::Beam | VfxEmitterShape::Ribbon => return,
    };

    let c0 = em.color_ramp_rgba[0];
    let norm_hdr = c0[0].max(c0[1]).max(c0[2]).max(1.0);
    let start_size = em.size_ramp[0].max(0.04);
    let end_size = em.size_ramp[2].max(em.size_ramp[1] * 0.35).max(0.02);

    burst_buf.emit_with_drag(
        GpuParticleInstance {
            position: world_origin + pos_offset,
            velocity: dir * speed,
            age: 0.0,
            lifetime,
            gravity: em.gravity,
            start_size,
            end_size,
            color_rgba: [
                (c0[0] / norm_hdr).clamp(0.0, 1.0),
                (c0[1] / norm_hdr).clamp(0.0, 1.0),
                (c0[2] / norm_hdr).clamp(0.0, 1.0),
                c0[3].clamp(0.05, 1.0),
            ],
        },
        em.drag.max(0.0),
    );
}

pub(crate) fn step_vfx_instance(
    inst: &mut ActiveDataDrivenVfxInstance,
    dt: f32,
    cam_forward: Vec3,
    burst_buf: &mut GpuParticleBurstBuffer,
    beam_batch: &mut VfxBeamAndRingBatchBuilder,
) -> bool {
    inst.age_secs += dt;
    if inst.age_secs >= inst.duration_secs {
        if inst.looping {
            inst.age_secs %= inst.duration_secs.max(0.08);
            for em in &mut inst.emitters {
                em.reset_cycle();
            }
        } else {
            return false;
        }
    }

    for (e_idx, em_rt) in inst.emitters.iter_mut().enumerate() {
        let em_dur = em_rt.def.duration_secs.max(0.08);
        em_rt.age_secs += dt;
        if em_rt.age_secs >= em_dur && (em_rt.def.looping || inst.looping) {
            em_rt.reset_cycle();
        }
        let active_window = em_rt.age_secs <= em_dur || em_rt.def.looping || inst.looping;
        let em_origin = inst.origin + Vec3::from_array(em_rt.def.local_offset);
        let norm_t = (em_rt.age_secs / em_dur).clamp(0.0, 1.0);
        let e_seed = inst.seed.wrapping_add((e_idx as u64 + 1) * 7919);

        match em_rt.def.shape {
            VfxEmitterShape::Beam => {
                if active_window {
                    let col = sample_color_ramp(&em_rt.def.color_ramp_rgba, norm_t);
                    let half_w = (em_rt.def.beam_width.max(0.14)) * 0.5;
                    let outer = [
                        (col[0] * 0.65).clamp(0.0, 1.0),
                        (col[1] * 0.65).clamp(0.0, 1.0),
                        (col[2] * 0.65).clamp(0.0, 1.0),
                        (col[3] * 0.85).clamp(0.0, 1.0),
                    ];
                    let core = [1.0, 0.98, 0.96, col[3].clamp(0.0, 1.0)];
                    beam_batch.push_3d_beam_segment(
                        em_origin,
                        inst.target_pos,
                        cam_forward,
                        half_w,
                        outer,
                    );
                    beam_batch.push_3d_beam_segment(
                        em_origin,
                        inst.target_pos,
                        cam_forward,
                        half_w * 0.42,
                        core,
                    );
                }
            }
            VfxEmitterShape::Ribbon => {
                let should_push = em_rt
                    .ribbon_trail
                    .back()
                    .map(|last| last.distance_squared(em_origin) >= 0.04)
                    .unwrap_or(true);
                if should_push {
                    if em_rt.ribbon_trail.len() >= MAX_RIBBON_POINTS {
                        em_rt.ribbon_trail.pop_front();
                    }
                    em_rt.ribbon_trail.push_back(em_origin);
                }
                if em_rt.ribbon_trail.len() >= 2 {
                    let base_w = em_rt.def.ribbon_width.max(0.14) * 0.5;
                    let len = em_rt.ribbon_trail.len();
                    for seg in 0..(len - 1) {
                        let tail_t = 1.0 - (seg as f32 / (len - 1).max(1) as f32);
                        let col = sample_color_ramp(&em_rt.def.color_ramp_rgba, tail_t);
                        let w = (base_w * (1.0 - tail_t * 0.65)).max(0.02);
                        beam_batch.push_3d_beam_segment(
                            em_rt.ribbon_trail[seg],
                            em_rt.ribbon_trail[seg + 1],
                            cam_forward,
                            w,
                            [
                                col[0].clamp(0.0, 1.0),
                                col[1].clamp(0.0, 1.0),
                                col[2].clamp(0.0, 1.0),
                                col[3],
                            ],
                        );
                    }
                }
            }
            shape => {
                if active_window && !em_rt.burst_emitted {
                    em_rt.burst_emitted = true;
                    for b in 0..em_rt.def.burst_count.min(96) {
                        emit_particle_from_emitter(
                            &em_rt.def,
                            em_origin,
                            inst.forward,
                            e_seed,
                            b as u64,
                            burst_buf,
                        );
                    }
                }
                if active_window && em_rt.def.continuous_rate_per_sec > 0.0 {
                    em_rt.continuous_accum += em_rt.def.continuous_rate_per_sec.min(180.0) * dt;
                    let emit_n = (em_rt.continuous_accum.floor() as u32).min(24);
                    em_rt.continuous_accum -= emit_n as f32;
                    let salt_base = (em_rt.age_secs * 1000.0) as u64 + 101;
                    for c in 0..emit_n {
                        emit_particle_from_emitter(
                            &em_rt.def,
                            em_origin,
                            inst.forward,
                            e_seed,
                            salt_base + c as u64,
                            burst_buf,
                        );
                    }
                }
                if shape == VfxEmitterShape::Ring && active_window {
                    let col = sample_color_ramp(&em_rt.def.color_ramp_rgba, norm_t);
                    let r = 0.25 + em_rt.def.sphere_radius.max(1.2) * norm_t;
                    beam_batch.push_ground_ring(
                        em_origin,
                        r,
                        0.12 * (1.0 - norm_t * 0.5),
                        20,
                        [
                            col[0].clamp(0.0, 1.0),
                            col[1].clamp(0.0, 1.0),
                            col[2].clamp(0.0, 1.0),
                            col[3],
                        ],
                    );
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sdk::schema::VfxDecalLinkSpec;

    #[test]
    fn multi_emitter_vfx_effect_spawns_particles_stamps_decal_and_adds_camera_trauma() {
        let mut runtime = DataDrivenVfxRuntimeState::default();
        let mut decals = DecalSystemState::default();
        let mut rig = RtsCameraRig {
            focus: Vec3::ZERO,
            ..RtsCameraRig::default()
        };
        let mut pool = GpuParticleBurstBuffer::default();
        let mut beams = VfxBeamAndRingBatchBuilder::default();

        let def = VfxEffectDef {
            id: "plasma_impact".to_string(),
            name: "Plasma Impact".to_string(),
            duration_secs: 1.0,
            looping: false,
            camera_shake_intensity: 0.45,
            camera_shake_radius: 30.0,
            ground_decal: Some(VfxDecalLinkSpec {
                decal_kind: "crater".to_string(),
                radius: 2.8,
                fade_time_secs: 20.0,
            }),
            fallback_style: ProjectileStyle::Laser,
            emitters: vec![
                VfxEmitterDef {
                    id: "sparks".to_string(),
                    shape: VfxEmitterShape::Cone,
                    burst_count: 18,
                    ..Default::default()
                },
                VfxEmitterDef {
                    id: "shock_ring".to_string(),
                    shape: VfxEmitterShape::Ring,
                    burst_count: 6,
                    sphere_radius: 2.4,
                    ..Default::default()
                },
            ],
        };

        runtime.spawn_effect(
            &def,
            Vec3::new(4.0, 0.5, 3.0),
            Vec3::new(4.0, 2.5, 3.0),
            None,
            None,
            Some(&mut decals),
            Some(&mut rig),
        );
        assert_eq!(decals.vfx_stamp_ring.len(), 1);
        assert!(rig.trauma > 0.25);

        let alive = step_vfx_instance(
            &mut runtime.active_instances[0],
            0.05,
            Vec3::NEG_Z,
            &mut pool,
            &mut beams,
        );
        assert!(alive);
        assert_eq!(pool.instances.len(), 24);
    }
}
