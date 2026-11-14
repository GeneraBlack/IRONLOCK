use super::signature_beams_waves::{
    emit_psychic_wave_geometry, emit_rad_beam_geometry, emit_sonic_wave_geometry,
};
use super::EnergyBeamVfx;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rts_sdk::schema::ProjectileStyle;

/// Authentic Red Alert 2 / Yuri's Revenge signature energy beam archetype (`Phase V6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SignatureBeamKind {
    /// Allied Prism Tower / Prism Tank / Mirage / Laser (`wid_low.contains("prism")` / `ProjectileStyle::Laser`).
    #[default]
    Prism,
    /// Soviet Tesla Coil / Tesla Tank / Tesla Trooper (`is_tesla` / `ProjectileStyle::Tesla`).
    Tesla,
    /// Iraqi Desolator / Radiation Cannon (`radbeam` / `ProjectileStyle::Radiation`).
    RadBeam,
    /// Yuri / Yuri Prime / Mastermind / Psychic Tower (`psy` / `psi` / `ProjectileStyle::MindControl`).
    PsychicWave,
    /// Allied Dolphin Sonic Pulse / Magnetron Telekinetic Wave (`ProjectileStyle::Sonic`).
    SonicWave,
}

/// Classifies a combat weapon into an authentic RA2 signature energy beam archetype, or `None` for
/// conventional ballistic shells, bullets, and missiles.
pub fn classify_signature_beam(
    weapon_id: &str,
    style: ProjectileStyle,
) -> Option<SignatureBeamKind> {
    let wid_low = weapon_id.trim().to_ascii_lowercase();
    if style == ProjectileStyle::Tesla || wid_low.contains("tesla") {
        Some(SignatureBeamKind::Tesla)
    } else if style == ProjectileStyle::Radiation
        || wid_low.contains("radbeam")
        || wid_low.contains("rad_beam")
    {
        Some(SignatureBeamKind::RadBeam)
    } else if style == ProjectileStyle::MindControl
        || wid_low.contains("psy")
        || wid_low.contains("psi")
        || wid_low.contains("mind")
    {
        Some(SignatureBeamKind::PsychicWave)
    } else if style == ProjectileStyle::Sonic
        || wid_low.contains("sonic")
        || wid_low.contains("magnet")
    {
        Some(SignatureBeamKind::SonicWave)
    } else if style == ProjectileStyle::Laser
        || wid_low.contains("prism")
        || wid_low.contains("laser")
    {
        Some(SignatureBeamKind::Prism)
    } else {
        None
    }
}

/// Returns `(flash_color, intensity_lumens, range_meters, lifetime_secs)` for impact dynamic lights.
pub fn signature_beam_impact_light_spec(kind: SignatureBeamKind) -> (Color, f32, f32, f32) {
    match kind {
        SignatureBeamKind::Prism => (Color::srgb(0.32, 0.95, 1.0), 105_000.0, 14.5, 0.24),
        SignatureBeamKind::Tesla => (Color::srgb(0.28, 0.78, 1.0), 125_000.0, 15.5, 0.24),
        SignatureBeamKind::RadBeam => (Color::srgb(0.18, 1.0, 0.14), 98_000.0, 15.0, 0.46),
        SignatureBeamKind::PsychicWave => (Color::srgb(0.92, 0.22, 1.0), 92_000.0, 14.0, 0.36),
        SignatureBeamKind::SonicWave => (Color::srgb(0.55, 0.92, 1.0), 82_000.0, 13.0, 0.22),
    }
}

/// Resolves deterministic Prism starburst splinter ray count (`2..=4` rays).
#[inline]
pub fn effective_prism_splinter_count(configured: u8, seed: u64) -> usize {
    if configured == 0 {
        2 + (seed % 3) as usize
    } else {
        (configured as usize).clamp(2, 4)
    }
}

/// Resolves deterministic Soviet Tesla braided lightning strand count (`2..=3` strands).
#[inline]
pub fn effective_tesla_strand_count(configured: u8, seed: u64) -> usize {
    if configured == 0 {
        2 + (seed & 1) as usize
    } else {
        (configured as usize).clamp(2, 3)
    }
}

#[inline]
fn det_u64(seed: u64, salt: u64) -> u64 {
    let mut x = seed
        .wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 30)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[inline]
fn det_unit01(seed: u64, salt: u64) -> f32 {
    (det_u64(seed, salt) & 0xFFFF) as f32 / 65535.0
}

#[inline]
fn det_signed(seed: u64, salt: u64) -> f32 {
    det_unit01(seed, salt) * 2.0 - 1.0
}

#[inline]
pub(super) fn orthonormal_beam_basis(start: Vec3, end: Vec3) -> Option<(Vec3, Vec3, Vec3, f32)> {
    let delta = end - start;
    let dist = delta.length();
    if !dist.is_finite() || dist < 1e-4 {
        return None;
    }
    let dir = delta / dist;
    let ref_up = if dir.y.abs() > 0.92 { Vec3::X } else { Vec3::Y };
    let mut right = dir.cross(ref_up).normalize_or_zero();
    if right.length_squared() < 1e-4 {
        right = Vec3::Z;
    }
    let up = right.cross(dir).normalize_or_zero();
    Some((dir, right, up, dist))
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

/// Batched 3D crossed-beam, ribbon, and ground-ring mesh builder shared by combat VFX and Studio previews.
#[derive(Default)]
pub struct VfxBeamAndRingBatchBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl VfxBeamAndRingBatchBuilder {
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    #[inline]
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    #[inline]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    #[inline]
    pub fn colors(&self) -> &[[f32; 4]] {
        &self.colors
    }

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

/// Emits batched 3D geometry for all RA2 signature energy beam archetypes (`Phase V6`).
pub fn emit_signature_beam_geometry(
    beam: &EnergyBeamVfx,
    cam_forward: Vec3,
    batch: &mut VfxBeamAndRingBatchBuilder,
) {
    let life = beam.lifetime.max(0.05);
    let norm_age = (beam.age / life).clamp(0.0, 1.0);
    let alpha = (1.0 - norm_age).clamp(0.0, 1.0);
    if alpha <= 0.005 {
        return;
    }

    let resolved_kind = if beam.is_tesla {
        SignatureBeamKind::Tesla
    } else {
        beam.beam_kind
    };

    match resolved_kind {
        SignatureBeamKind::Prism => {
            emit_prism_beam_and_starburst(beam, norm_age, alpha, cam_forward, batch)
        }
        SignatureBeamKind::Tesla => {
            emit_tesla_multi_arc_geometry(beam, norm_age, alpha, cam_forward, batch)
        }
        SignatureBeamKind::RadBeam => {
            emit_rad_beam_geometry(beam, norm_age, alpha, cam_forward, batch)
        }
        SignatureBeamKind::PsychicWave => {
            emit_psychic_wave_geometry(beam, norm_age, alpha, cam_forward, batch)
        }
        SignatureBeamKind::SonicWave => {
            emit_sonic_wave_geometry(beam, norm_age, alpha, cam_forward, batch)
        }
    }
}

/// Allied Prism Beam: white-hot inner core + brilliant cyan/azure outer halo, emitter crystal
/// charge flare at `beam.start`, and `2..=4` shorter refracted crystalline starburst rays at `beam.end`.
fn emit_prism_beam_and_starburst(
    beam: &EnergyBeamVfx,
    norm_age: f32,
    alpha: f32,
    cam_forward: Vec3,
    batch: &mut VfxBeamAndRingBatchBuilder,
) {
    let Some((_, right, up, dist)) = orthonormal_beam_basis(beam.start, beam.end) else {
        return;
    };
    let shimmer = 1.0 + 0.14 * (norm_age * std::f32::consts::TAU * 3.0).sin();

    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.175 * shimmer,
        [0.10, 0.68, 1.0, alpha * 0.45],
    );
    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.095 * shimmer,
        [0.26, 0.94, 1.0, alpha * 0.86],
    );
    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.038 * shimmer,
        [0.98, 1.0, 1.0, alpha],
    );

    let charge_env = (1.0 - norm_age * 1.15).clamp(0.0, 1.0);
    if charge_env > 0.04 {
        let flare_r = 0.42 * (0.55 + 0.45 * charge_env);
        for axis in [right, up, (right + up).normalize_or_zero()] {
            let a = beam.start - axis * flare_r;
            let b = beam.start + axis * flare_r;
            batch.push_3d_beam_segment(
                a,
                b,
                cam_forward,
                0.075 * charge_env,
                [0.24, 0.92, 1.0, charge_env * 0.82],
            );
            batch.push_3d_beam_segment(
                a,
                b,
                cam_forward,
                0.028 * charge_env,
                [1.0, 1.0, 1.0, charge_env],
            );
        }
    }

    let ray_count = effective_prism_splinter_count(beam.splinter_count, beam.seed);
    let base_yaw = det_unit01(beam.seed, 11) * std::f32::consts::TAU;
    let max_splinter_len = (dist * 0.55).clamp(0.85, 2.55);

    for r in 0..ray_count {
        let salt = (r as u64 + 1) * 37;
        let yaw = base_yaw
            + (r as f32 / ray_count as f32) * std::f32::consts::TAU
            + det_signed(beam.seed, salt) * 0.28;
        let ray_len = (0.58 + 0.42 * det_unit01(beam.seed, salt + 3)) * max_splinter_len;
        let elev = 0.22 + 0.42 * det_unit01(beam.seed, salt + 7);
        let (sy, cy) = yaw.sin_cos();
        let horiz = Vec3::new(cy, 0.0, sy);

        let mid = beam.end + horiz * (ray_len * 0.52) + Vec3::Y * (elev + 0.26);
        let tip = beam.end + horiz * ray_len + Vec3::Y * (elev * 0.45);

        batch.push_3d_beam_segment(
            beam.end,
            mid,
            cam_forward,
            0.068,
            [0.22, 0.90, 1.0, alpha * 0.80],
        );
        batch.push_3d_beam_segment(
            beam.end,
            mid,
            cam_forward,
            0.026,
            [0.97, 1.0, 1.0, alpha * 0.96],
        );
        batch.push_3d_beam_segment(
            mid,
            tip,
            cam_forward,
            0.046,
            [0.18, 0.84, 1.0, alpha * 0.68],
        );
        batch.push_3d_beam_segment(mid, tip, cam_forward, 0.018, [0.95, 1.0, 1.0, alpha * 0.88]);
    }

    batch.push_ground_ring(
        beam.end,
        0.28 + norm_age * 1.35,
        0.10 * (1.0 - norm_age * 0.45),
        18,
        [0.30, 0.94, 1.0, alpha * 0.74],
    );
}

/// Soviet Multi-Strand Jagged Tesla Arcs: 2–3 braided/branching electric blue-white lightning bolts
/// with deterministic per-frame crackle jitter (`~38 Hz`), coil corona flare at `beam.start`, and impact arc.
fn emit_tesla_multi_arc_geometry(
    beam: &EnergyBeamVfx,
    norm_age: f32,
    alpha: f32,
    cam_forward: Vec3,
    batch: &mut VfxBeamAndRingBatchBuilder,
) {
    let Some((_, right, up, dist)) = orthonormal_beam_basis(beam.start, beam.end) else {
        return;
    };
    let strand_count = effective_tesla_strand_count(beam.splinter_count, beam.seed);
    let segments = (dist * 0.65).round().clamp(7.0, 11.0) as usize;
    let crackle_step = (beam.age.max(0.0) * 38.0).floor() as u64;
    let crackle_seed = beam
        .seed
        .wrapping_add(crackle_step.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let jitter_scale = (0.28 + dist.min(18.0) * 0.025).min(0.76);

    let discharge_env = (1.0 - norm_age * 0.9).clamp(0.0, 1.0);
    for c in 0..4u64 {
        let csalt = 700 + c * 29;
        let dir_sp = (right * det_signed(crackle_seed, csalt)
            + up * det_signed(crackle_seed, csalt + 1)
            + Vec3::Y * 0.35)
            .normalize_or_zero();
        let corona_tip = beam.start + dir_sp * (0.36 + 0.24 * det_unit01(crackle_seed, csalt + 2));
        batch.push_3d_beam_segment(
            beam.start,
            corona_tip,
            cam_forward,
            0.072 * discharge_env,
            [0.22, 0.72, 1.0, alpha * 0.85],
        );
        batch.push_3d_beam_segment(
            beam.start,
            corona_tip,
            cam_forward,
            0.026 * discharge_env,
            [0.94, 0.99, 1.0, alpha],
        );
    }

    for k in 0..strand_count {
        let is_primary = k == 0;
        let phase_base =
            (k as f32 / strand_count as f32) * std::f32::consts::TAU + crackle_step as f32 * 0.95;
        let outer_w = if is_primary { 0.115 } else { 0.078 };
        let core_w = if is_primary { 0.044 } else { 0.028 };
        let outer_col = if is_primary {
            [0.20, 0.68, 1.0, alpha * 0.88]
        } else {
            [0.26, 0.76, 1.0, alpha * 0.76]
        };
        let core_col = [0.93, 0.99, 1.0, alpha * if is_primary { 1.0 } else { 0.92 }];

        let mut prev = beam.start;
        for s in 1..=segments {
            let t = s as f32 / segments as f32;
            let pt = if s == segments {
                beam.end
            } else {
                let env = (t * std::f32::consts::PI).sin();
                let braid_ang = t * std::f32::consts::TAU * 1.45 + phase_base;
                let braid_r = (0.14 + 0.05 * k as f32) * env;
                let salt = (k as u64 * 131) + (s as u64 * 17);
                let j_right = det_signed(crackle_seed, salt) * jitter_scale * env;
                let j_up = det_signed(crackle_seed, salt + 5) * jitter_scale * 0.85 * env;
                beam.start.lerp(beam.end, t)
                    + right * (braid_ang.cos() * braid_r + j_right)
                    + up * (braid_ang.sin() * braid_r + j_up)
                    + Vec3::Y * (0.20 * env)
            };

            batch.push_3d_beam_segment(prev, pt, cam_forward, outer_w, outer_col);
            batch.push_3d_beam_segment(prev, pt, cam_forward, core_w, core_col);

            if s < segments && ((is_primary && (s == 3 || s == 6)) || (!is_primary && s == 4)) {
                let fsalt = 900 + (k as u64 * 53) + (s as u64 * 19);
                let fork_dir = ((pt - prev).normalize_or_zero() * 0.58
                    + right * det_signed(crackle_seed, fsalt) * 0.85
                    + up * det_signed(crackle_seed, fsalt + 2) * 0.75)
                    .normalize_or_zero();
                let fork_len = 0.55 + det_unit01(crackle_seed, fsalt + 4) * 0.65;
                let fork_mid = pt + fork_dir * (fork_len * 0.54) + Vec3::Y * 0.08;
                let fork_tip = pt + fork_dir * fork_len - Vec3::Y * 0.14;
                batch.push_3d_beam_segment(
                    pt,
                    fork_mid,
                    cam_forward,
                    0.052,
                    [0.26, 0.76, 1.0, alpha * 0.74],
                );
                batch.push_3d_beam_segment(
                    pt,
                    fork_mid,
                    cam_forward,
                    0.020,
                    [0.92, 0.98, 1.0, alpha * 0.88],
                );
                batch.push_3d_beam_segment(
                    fork_mid,
                    fork_tip,
                    cam_forward,
                    0.032,
                    [0.22, 0.66, 1.0, alpha * 0.48],
                );
            }
            prev = pt;
        }
    }

    batch.push_ground_ring(
        beam.end,
        0.30 + norm_age * 1.45,
        0.11 * (1.0 - norm_age * 0.4),
        16,
        [0.28, 0.78, 1.0, alpha * 0.82],
    );
}
