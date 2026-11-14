use super::signature_beams::{orthonormal_beam_basis, VfxBeamAndRingBatchBuilder};
use super::EnergyBeamVfx;
use bevy::prelude::*;

/// Iraqi Desolator Rad-Beam: vivid neon-emerald (`srgb(0.18, 1.0, 0.14)`) pulsing core wrapped
/// in a 3D helical radioactive wave + emerald ground radiation ring glow at impact.
pub(super) fn emit_rad_beam_geometry(
    beam: &EnergyBeamVfx,
    norm_age: f32,
    alpha: f32,
    cam_forward: Vec3,
    batch: &mut VfxBeamAndRingBatchBuilder,
) {
    let Some((_, right, up, _)) = orthonormal_beam_basis(beam.start, beam.end) else {
        return;
    };
    let pulse = 1.0 + 0.22 * (beam.age * 28.0).sin();

    // Central pulsing neon-emerald (`[0.18, 1.0, 0.14]`) core beam
    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.135 * pulse,
        [0.18, 1.0, 0.14, alpha * 0.78],
    );
    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.046 * pulse,
        [0.72, 1.0, 0.44, alpha],
    );

    // Spiraling neon-emerald helical wave wrapping around the primary beam
    let segments = 12usize;
    let mut prev_h = beam.start;
    for s in 1..=segments {
        let t = s as f32 / segments as f32;
        let env = (t * std::f32::consts::PI).sin();
        let theta = t * std::f32::consts::TAU * 2.5 - beam.age * 20.0;
        let radius = 0.19 * env;
        let curr_h = if s == segments {
            beam.end
        } else {
            beam.start.lerp(beam.end, t)
                + right * (theta.cos() * radius)
                + up * (theta.sin() * radius)
        };
        batch.push_3d_beam_segment(
            prev_h,
            curr_h,
            cam_forward,
            0.068,
            [0.18, 1.0, 0.14, alpha * 0.88],
        );
        batch.push_3d_beam_segment(
            prev_h,
            curr_h,
            cam_forward,
            0.024,
            [0.64, 1.0, 0.38, alpha * 0.94],
        );
        prev_h = curr_h;
    }

    // Immediate impact neon-emerald radiation ring glow (paired with lingering ground rings)
    let ground_impact = Vec3::new(beam.end.x, (beam.end.y - 0.85).max(0.06), beam.end.z);
    batch.push_ground_ring(
        ground_impact,
        0.55 + norm_age * 2.10,
        0.16 * (1.0 - norm_age * 0.35),
        22,
        [0.18, 1.0, 0.14, alpha * 0.84],
    );
}

/// Yuri Psychic Wave: pulsing magenta-violet (`srgb(0.92, 0.22, 1.0)`) concentric psionic wave
/// rings + upward-curved mental-tether arc between caster and target.
pub(super) fn emit_psychic_wave_geometry(
    beam: &EnergyBeamVfx,
    norm_age: f32,
    alpha: f32,
    cam_forward: Vec3,
    batch: &mut VfxBeamAndRingBatchBuilder,
) {
    let Some((_, right, _, dist)) = orthonormal_beam_basis(beam.start, beam.end) else {
        return;
    };
    let arc_lift = (0.85 + dist * 0.12).clamp(0.85, 2.75);
    let ctrl = (beam.start + beam.end) * 0.5 + Vec3::Y * arc_lift;

    // 1. Curved quadratic-Bezier mental-tether arc with gentle psionic lateral undulation
    let segments = 12usize;
    let mut prev = beam.start;
    for s in 1..=segments {
        let t = s as f32 / segments as f32;
        let omt = 1.0 - t;
        let bezier = beam.start * (omt * omt) + ctrl * (2.0 * omt * t) + beam.end * (t * t);
        let ripple = (t * std::f32::consts::TAU * 2.0 - beam.age * 16.0).sin()
            * 0.14
            * (t * std::f32::consts::PI).sin();
        let curr = if s == segments {
            beam.end
        } else {
            bezier + right * ripple
        };
        let pulse_w = 1.0 + 0.18 * (t * std::f32::consts::TAU - beam.age * 18.0).sin();
        batch.push_3d_beam_segment(
            prev,
            curr,
            cam_forward,
            0.105 * pulse_w,
            [0.92, 0.22, 1.0, alpha * 0.84],
        );
        batch.push_3d_beam_segment(
            prev,
            curr,
            cam_forward,
            0.038 * pulse_w,
            [1.0, 0.74, 1.0, alpha],
        );
        prev = curr;
    }

    // 2. Pulsing concentric magenta-violet (`[0.92, 0.22, 1.0]`) psionic wave rings at caster & target
    for ring_idx in 0..3usize {
        let phase = (norm_age * 1.45 + ring_idx as f32 / 3.0).fract();
        let ring_alpha = alpha * (1.0 - phase) * 0.86;
        if ring_alpha > 0.03 {
            batch.push_ground_ring(
                beam.start,
                0.22 + phase * 1.35,
                0.085,
                18,
                [0.92, 0.22, 1.0, ring_alpha],
            );
            batch.push_ground_ring(
                beam.end,
                0.28 + phase * 1.75,
                0.105,
                20,
                [0.92, 0.22, 1.0, ring_alpha],
            );
        }
    }
}

/// Allied Dolphin Sonic Pulse / Magnetron Levitation Wave: marching aquamarine wave rings + carrier beam.
pub(super) fn emit_sonic_wave_geometry(
    beam: &EnergyBeamVfx,
    norm_age: f32,
    alpha: f32,
    cam_forward: Vec3,
    batch: &mut VfxBeamAndRingBatchBuilder,
) {
    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.115,
        [0.52, 0.90, 1.0, alpha * 0.78],
    );
    batch.push_3d_beam_segment(
        beam.start,
        beam.end,
        cam_forward,
        0.042,
        [0.95, 1.0, 1.0, alpha],
    );
    for i in 0..3usize {
        let t = ((norm_age + i as f32 * 0.33).fract()).clamp(0.05, 0.98);
        let center = beam.start.lerp(beam.end, t);
        batch.push_ground_ring(
            center,
            0.25 + t * 0.95,
            0.08,
            16,
            [0.58, 0.94, 1.0, alpha * (1.0 - t * 0.5)],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::signature_beams::{
        classify_signature_beam, effective_prism_splinter_count, effective_tesla_strand_count,
        emit_signature_beam_geometry, SignatureBeamKind, VfxBeamAndRingBatchBuilder,
    };
    use super::super::EnergyBeamVfx;
    use bevy::prelude::Vec3;
    use rts_sdk::schema::ProjectileStyle;

    #[test]
    fn signature_beam_classifier_distinguishes_prism_tesla_radbeam_and_psychic_weapons() {
        assert_eq!(
            classify_signature_beam("prism_beam", ProjectileStyle::Laser),
            Some(SignatureBeamKind::Prism)
        );
        assert_eq!(
            classify_signature_beam("allied_prism_tower", ProjectileStyle::Bullet),
            Some(SignatureBeamKind::Prism)
        );
        assert_eq!(
            classify_signature_beam("tesla_bolt", ProjectileStyle::Tesla),
            Some(SignatureBeamKind::Tesla)
        );
        assert_eq!(
            classify_signature_beam("portable_tesla", ProjectileStyle::Bullet),
            Some(SignatureBeamKind::Tesla)
        );
        assert_eq!(
            classify_signature_beam("radbeam", ProjectileStyle::Bullet),
            Some(SignatureBeamKind::RadBeam)
        );
        assert_eq!(
            classify_signature_beam("rad_beam", ProjectileStyle::Radiation),
            Some(SignatureBeamKind::RadBeam)
        );
        assert_eq!(
            classify_signature_beam("psychic_tower_mind_beam", ProjectileStyle::MindControl),
            Some(SignatureBeamKind::PsychicWave)
        );
        assert_eq!(
            classify_signature_beam("psy_dominator", ProjectileStyle::Bullet),
            Some(SignatureBeamKind::PsychicWave)
        );
        assert_eq!(
            classify_signature_beam("sonic_pulse", ProjectileStyle::Sonic),
            Some(SignatureBeamKind::SonicWave)
        );
        assert_eq!(
            classify_signature_beam("cannon_120mm", ProjectileStyle::Bullet),
            None
        );
        assert_eq!(
            classify_signature_beam("v3_ballistic_rocket", ProjectileStyle::Missile),
            None
        );
    }

    #[test]
    fn prism_starburst_and_multi_arc_tesla_geometry_generate_braids_and_crackle_jitter() {
        let cam_fwd = Vec3::new(0.0, -0.7, -0.71).normalize();
        let prism = EnergyBeamVfx {
            start: Vec3::new(0.0, 2.4, 0.0),
            end: Vec3::new(10.0, 1.0, 4.0),
            is_tesla: false,
            style: ProjectileStyle::Laser,
            beam_kind: SignatureBeamKind::Prism,
            splinter_count: 3,
            age: 0.04,
            lifetime: 0.34,
            seed: 42,
        };
        let mut prism_batch = VfxBeamAndRingBatchBuilder::default();
        emit_signature_beam_geometry(&prism, cam_fwd, &mut prism_batch);
        assert!(prism_batch.vertex_count() >= 160);
        assert!(prism_batch
            .colors()
            .iter()
            .any(|c| c[0] > 0.95 && c[1] > 0.95 && c[2] > 0.95));

        for seed in 0..12u64 {
            let splinters = effective_prism_splinter_count(0, seed);
            assert!((2..=4).contains(&splinters));
            let strands = effective_tesla_strand_count(0, seed);
            assert!((2..=3).contains(&strands));
        }

        let mut tesla_f0 = EnergyBeamVfx {
            start: Vec3::new(2.0, 3.2, 2.0),
            end: Vec3::new(14.0, 1.0, 8.0),
            is_tesla: true,
            style: ProjectileStyle::Tesla,
            beam_kind: SignatureBeamKind::Tesla,
            splinter_count: 3,
            age: 0.01,
            lifetime: 0.34,
            seed: 1337,
        };
        let mut batch_t0 = VfxBeamAndRingBatchBuilder::default();
        let mut batch_t0_repeat = VfxBeamAndRingBatchBuilder::default();
        emit_signature_beam_geometry(&tesla_f0, cam_fwd, &mut batch_t0);
        emit_signature_beam_geometry(&tesla_f0, cam_fwd, &mut batch_t0_repeat);
        assert_eq!(batch_t0.positions(), batch_t0_repeat.positions());
        assert!(batch_t0.vertex_count() >= 280);

        // Advancing age across crackle quantum deterministically jitters interior lightning nodes
        tesla_f0.age = 0.09;
        let mut batch_t1 = VfxBeamAndRingBatchBuilder::default();
        emit_signature_beam_geometry(&tesla_f0, cam_fwd, &mut batch_t1);
        assert_ne!(batch_t0.positions(), batch_t1.positions());
    }

    #[test]
    fn rad_beam_and_psychic_wave_emit_exact_signature_colors_helix_and_curved_tether() {
        let cam_fwd = Vec3::NEG_Z;
        let rad = EnergyBeamVfx {
            start: Vec3::new(0.0, 1.2, 0.0),
            end: Vec3::new(12.0, 1.0, 0.0),
            is_tesla: false,
            style: ProjectileStyle::Radiation,
            beam_kind: SignatureBeamKind::RadBeam,
            splinter_count: 0,
            age: 0.05,
            lifetime: 0.38,
            seed: 99,
        };
        let mut rad_batch = VfxBeamAndRingBatchBuilder::default();
        emit_signature_beam_geometry(&rad, cam_fwd, &mut rad_batch);
        assert!(rad_batch.colors().iter().any(|c| {
            (c[0] - 0.18).abs() < 1e-3 && (c[1] - 1.0).abs() < 1e-3 && (c[2] - 0.14).abs() < 1e-3
        }));

        let psy = EnergyBeamVfx {
            start: Vec3::new(0.0, 1.2, 0.0),
            end: Vec3::new(12.0, 1.2, 0.0),
            is_tesla: false,
            style: ProjectileStyle::MindControl,
            beam_kind: SignatureBeamKind::PsychicWave,
            splinter_count: 0,
            age: 0.06,
            lifetime: 0.42,
            seed: 77,
        };
        let mut psy_batch = VfxBeamAndRingBatchBuilder::default();
        emit_signature_beam_geometry(&psy, cam_fwd, &mut psy_batch);
        assert!(psy_batch.colors().iter().any(|c| {
            (c[0] - 0.92).abs() < 1e-3 && (c[1] - 0.22).abs() < 1e-3 && (c[2] - 1.0).abs() < 1e-3
        }));
        let max_y = psy_batch
            .positions()
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max_y > 1.85,
            "Psychic mental tether must arch upward above linear 1.2m endpoints (got {max_y})"
        );
    }
}
