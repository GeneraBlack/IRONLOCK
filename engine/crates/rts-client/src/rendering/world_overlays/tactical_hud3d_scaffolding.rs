use super::batch_builder::DynamicQuadBatchBuilder;
use crate::rendering::Ra2EntityVisualMode;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;

/// Returns `true` when the loaded mod ships all 3 `v3_ultra` 3D PBR faction construction rigs
/// (`assets/models/fx/{allied,soviet,yuri}_construction_rig.glb`).
#[inline]
pub fn mod_has_3d_construction_rigs(loaded: &LoadedMod) -> bool {
    loaded.asset_exists("assets/models/fx/allied_construction_rig.glb")
        && loaded.asset_exists("assets/models/fx/soviet_construction_rig.glb")
        && loaded.asset_exists("assets/models/fx/yuri_construction_rig.glb")
}

/// Returns `true` when the 3D glTF construction rig system (`construction_rig_sync.rs`) owns
/// active scaffolding/cranes, suppressing the legacy unlit procedural HUD strut fallback.
#[inline]
pub fn should_use_3d_gltf_construction_rig(
    loaded: &LoadedMod,
    visual_mode: Ra2EntityVisualMode,
) -> bool {
    visual_mode == Ra2EntityVisualMode::Full3dMeshes && mod_has_3d_construction_rigs(loaded)
}

/// Emits growing procedural 3D steel construction scaffolding + animated slewing tower-crane boom
/// around buildings with `construction_progress < 1.0` (`Task 206` fallback for `HybridHdSprites`
/// and mods without dedicated `fx/*_construction_rig.glb` assets).
#[allow(clippy::too_many_arguments)]
pub fn push_building_construction_scaffolding_and_crane(
    solid_batch: &mut DynamicQuadBatchBuilder,
    center: Vec3,
    half_w: f32,
    half_d: f32,
    full_height: f32,
    progress_01: f32,
    elapsed_secs: f32,
    team_rgb: [f32; 3],
) {
    let prog = progress_01.clamp(0.0, 1.0);
    if prog >= 0.999 {
        return;
    }
    let hw = half_w.max(1.1);
    let hd = half_d.max(1.1);
    let max_h = full_height.max(2.6);
    let scaffold_h = (max_h * (0.20 + 0.80 * prog)).max(0.75);
    let base_y = center.y;
    let top_y = base_y + scaffold_h;

    let steel_col = [0.54, 0.59, 0.65, 1.0];
    let brace_col = [0.42, 0.47, 0.53, 1.0];
    let crane_col = [
        (0.72 + team_rgb[0] * 0.28).clamp(0.0, 1.0),
        (0.62 + team_rgb[1] * 0.24).clamp(0.0, 1.0),
        (0.18 + team_rgb[2] * 0.22).clamp(0.0, 1.0),
        1.0,
    ];

    let corners = [
        Vec3::new(center.x - hw, base_y, center.z - hd),
        Vec3::new(center.x + hw, base_y, center.z - hd),
        Vec3::new(center.x + hw, base_y, center.z + hd),
        Vec3::new(center.x - hw, base_y, center.z + hd),
    ];

    for c in corners {
        solid_batch.push_strut_3d(c, Vec3::new(c.x, top_y, c.z), 0.065, steel_col);
    }

    let tiers = if prog < 0.34 {
        1usize
    } else if prog < 0.68 {
        2usize
    } else {
        3usize
    };
    let tier_h = scaffold_h / (tiers as f32);
    for t in 0..tiers {
        let y0 = base_y + (t as f32) * tier_h;
        let y1 = base_y + ((t + 1) as f32) * tier_h;
        for edge in 0..4 {
            let a = corners[edge];
            let b = corners[(edge + 1) % 4];
            let a_top = Vec3::new(a.x, y1, a.z);
            let b_top = Vec3::new(b.x, y1, b.z);
            let a_bot = Vec3::new(a.x, y0, a.z);
            let b_bot = Vec3::new(b.x, y0, b.z);
            solid_batch.push_strut_3d(a_top, b_top, 0.048, steel_col);
            solid_batch.push_strut_3d(a_bot, b_top, 0.034, brace_col);
            solid_batch.push_strut_3d(b_bot, a_top, 0.034, brace_col);
        }
    }

    let mast_base = Vec3::new(center.x + hw + 0.34, base_y, center.z - hd - 0.34);
    let mast_top_y = base_y + max_h + 1.45;
    let mast_top = Vec3::new(mast_base.x, mast_top_y, mast_base.z);
    solid_batch.push_oriented_box(
        Vec3::new(mast_base.x, base_y + 0.14, mast_base.z),
        Vec3::new(0.28, 0.14, 0.28),
        Quat::IDENTITY,
        brace_col,
    );
    solid_batch.push_strut_3d(mast_base, mast_top, 0.095, crane_col);

    let phase = center.x * 0.37 + center.z * 0.23;
    let to_center =
        Vec3::new(center.x - mast_base.x, 0.0, center.z - mast_base.z).normalize_or(Vec3::NEG_X);
    let sweep = (elapsed_secs * 1.15 + phase).sin() * 0.58;
    let slew_rot = Quat::from_rotation_y(sweep);
    let boom_dir = (slew_rot * to_center).normalize_or(Vec3::NEG_X);

    let boom_reach = (hw.hypot(hd) * 1.45).clamp(2.2, 6.8);
    let jib_tip = mast_top + boom_dir * boom_reach;
    let counter_tip = mast_top - boom_dir * (boom_reach * 0.36);
    let apex = mast_top + Vec3::Y * 0.68;

    solid_batch.push_strut_3d(counter_tip, jib_tip, 0.068, crane_col);
    solid_batch.push_strut_3d(apex, jib_tip, 0.032, steel_col);
    solid_batch.push_strut_3d(apex, counter_tip, 0.036, steel_col);
    solid_batch.push_oriented_box(
        counter_tip - Vec3::Y * 0.14,
        Vec3::new(0.20, 0.16, 0.20),
        Quat::IDENTITY,
        [0.32, 0.35, 0.39, 1.0],
    );

    let trolley_t = 0.52 + (elapsed_secs * 1.7 + phase).cos() * 0.26;
    let trolley_pt = mast_top + boom_dir * (boom_reach * trolley_t);
    let hook_pt = Vec3::new(trolley_pt.x, top_y.min(mast_top_y - 0.35), trolley_pt.z);
    solid_batch.push_strut_3d(trolley_pt, hook_pt, 0.024, [0.22, 0.25, 0.28, 1.0]);
    let spark_pulse = 0.65 + (elapsed_secs * 18.0 + phase).sin().abs() * 0.35;
    solid_batch.push_oriented_box(
        hook_pt,
        Vec3::splat(0.11 * spark_pulse),
        Quat::IDENTITY,
        [1.0, 0.84, 0.28, 1.0],
    );
}

/// Emits a 3D allied team-ping vertical light pillar + floating beacon + expanding ground shockwave rings (`Task 207-3D`).
pub fn push_tactical_ping_pillar_and_rings(
    ground_batch: &mut DynamicQuadBatchBuilder,
    billboard_batch: &mut DynamicQuadBatchBuilder,
    world_pos: Vec3,
    remaining_secs: f32,
    team_rgb: [f32; 3],
    cam_right: Vec3,
    cam_up: Vec3,
) {
    let total_secs = 4.5_f32;
    let age = (total_secs - remaining_secs).max(0.0);
    let life_a = (remaining_secs / total_secs).clamp(0.18, 0.96);
    let p_pos = Vec3::new(world_pos.x, world_pos.y.max(0.04) + 0.10, world_pos.z);

    for ring_idx in 0..3 {
        let phase = ((age * 1.55 + ring_idx as f32 * 0.33) % 1.0).clamp(0.0, 1.0);
        let radius = 0.65 + phase * 3.85;
        let ring_alpha = (1.0 - phase) * life_a * 0.92;
        ground_batch.push_ground_ring(
            p_pos,
            radius,
            0.13,
            24,
            [
                (team_rgb[0] * 0.72 + 0.28).clamp(0.0, 1.0),
                (team_rgb[1] * 0.72 + 0.28).clamp(0.0, 1.0),
                (team_rgb[2] * 0.72 + 0.28).clamp(0.0, 1.0),
                ring_alpha.clamp(0.08, 0.95),
            ],
        );
    }
    let cross_col = [1.0, 0.96, 0.76, life_a * 0.90];
    ground_batch.push_ground_line(
        p_pos - Vec3::X * 0.78,
        p_pos + Vec3::X * 0.78,
        0.055,
        cross_col,
    );
    ground_batch.push_ground_line(
        p_pos - Vec3::Z * 0.78,
        p_pos + Vec3::Z * 0.78,
        0.055,
        cross_col,
    );

    let pillar_h = 14.5_f32;
    let outer_rgba = [team_rgb[0], team_rgb[1], team_rgb[2], life_a * 0.42];
    let inner_rgba = [
        (team_rgb[0] * 0.35 + 0.65).clamp(0.0, 1.0),
        (team_rgb[1] * 0.35 + 0.65).clamp(0.0, 1.0),
        (team_rgb[2] * 0.35 + 0.65).clamp(0.0, 1.0),
        life_a * 0.84,
    ];
    for &(axis, nrm) in &[(Vec3::X, Vec3::Z), (Vec3::Z, Vec3::X)] {
        let hw_outer = 0.46;
        let hw_top = 0.18;
        billboard_batch.push_quad(
            p_pos - axis * hw_outer,
            p_pos + axis * hw_outer,
            p_pos + Vec3::Y * pillar_h + axis * hw_top,
            p_pos + Vec3::Y * pillar_h - axis * hw_top,
            nrm,
            outer_rgba,
        );
        let hw_core = 0.15;
        billboard_batch.push_quad(
            p_pos - axis * hw_core,
            p_pos + axis * hw_core,
            p_pos + Vec3::Y * (pillar_h * 0.92) + axis * (hw_core * 0.45),
            p_pos + Vec3::Y * (pillar_h * 0.92) - axis * (hw_core * 0.45),
            nrm,
            inner_rgba,
        );
    }

    let bob = (age * 4.2).sin() * 0.32;
    let beacon_c = p_pos + Vec3::Y * (3.6 + bob);
    let fwd = cam_right.cross(cam_up).normalize_or_zero();
    let dr = cam_right * 0.42;
    let du = cam_up * 0.56;
    billboard_batch.push_quad(
        beacon_c - du,
        beacon_c + dr,
        beacon_c + du,
        beacon_c - dr,
        fwd,
        [1.0, 0.95, 0.62, life_a],
    );
}
