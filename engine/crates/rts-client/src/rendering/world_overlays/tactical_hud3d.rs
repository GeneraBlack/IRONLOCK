use super::batch_builder::{cover_level_shield_rgba, DynamicQuadBatchBuilder};
use crate::rendering::{resolve_accessible_team_color, CachedFogVisibility, ColorblindPaletteMode};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;
use rts_simulation::world::{
    CoverLevel, EntityKind, SimEntity, SimWorld, TileVisibility, VeterancyRank,
};
use std::collections::BTreeSet;

/// Emits growing procedural 3D steel construction scaffolding + animated slewing tower-crane boom
/// around buildings with `construction_progress < 1.0` (`Task 206`).
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

/// Renders 3D steel Warren-truss bridge spans (`world.grid.bridge_tiles`), demolished abutment stubs
/// (`world.destroyed_bridges`), and bankside `BridgeRepairHut` status beacons (`Task 214-3D`).
#[allow(clippy::too_many_arguments)]
pub fn push_bridges_and_repair_huts(
    solid_batch: &mut DynamicQuadBatchBuilder,
    ground_batch: &mut DynamicQuadBatchBuilder,
    billboard_batch: &mut DynamicQuadBatchBuilder,
    world: &SimWorld,
    loaded: &LoadedMod,
    cached_fow: &CachedFogVisibility,
    elapsed_secs: f32,
    cam_right: Vec3,
    cam_up: Vec3,
) {
    let ts = world.grid.tile_size.max(1.0);
    let deck_y = rts_simulation::pathfinding::CollisionGrid::BRIDGE_DECK_Y;
    let water_y = rts_simulation::pathfinding::CollisionGrid::WATER_SURFACE_Y;

    let deck_col = [0.36, 0.39, 0.43, 1.0];
    let truss_col = [0.52, 0.56, 0.62, 1.0];
    let curb_col = [0.84, 0.68, 0.18, 1.0];
    let pier_col = [0.30, 0.33, 0.37, 1.0];

    for &btile in &world.grid.bridge_tiles {
        if cached_fow.tile_visibility(btile) == TileVisibility::Unexplored {
            continue;
        }
        let tp = world.grid.tile_to_world(btile);
        let center = Vec3::new(tp[0], deck_y - 0.08, tp[2]);
        solid_batch.push_oriented_box(
            center,
            Vec3::new(ts * 0.50, 0.09, ts * 0.50),
            Quat::IDENTITY,
            deck_col,
        );

        if ((btile[0] + btile[1]).rem_euclid(2)) == 0 && world.grid.water_tiles.contains(&btile) {
            let pier_h = ((deck_y - 0.14) - (water_y - 0.45)).max(0.4) * 0.5;
            let pier_y = (water_y - 0.45) + pier_h;
            for sx in [-1.0_f32, 1.0_f32] {
                solid_batch.push_oriented_box(
                    Vec3::new(tp[0] + sx * (ts * 0.36), pier_y, tp[2]),
                    Vec3::new(0.14, pier_h, 0.14),
                    Quat::IDENTITY,
                    pier_col,
                );
            }
        }

        let edges: [([i32; 2], Vec3, Vec3); 4] = [
            (
                [btile[0], btile[1] - 1],
                Vec3::new(0.0, 0.0, -ts * 0.46),
                Vec3::X,
            ),
            (
                [btile[0] + 1, btile[1]],
                Vec3::new(ts * 0.46, 0.0, 0.0),
                Vec3::Z,
            ),
            (
                [btile[0], btile[1] + 1],
                Vec3::new(0.0, 0.0, ts * 0.46),
                Vec3::X,
            ),
            (
                [btile[0] - 1, btile[1]],
                Vec3::new(-ts * 0.46, 0.0, 0.0),
                Vec3::Z,
            ),
        ];
        for (ntile, flank_off, span_axis) in edges {
            let is_lateral_water_edge = !world.grid.bridge_tiles.contains(&ntile)
                && (world.grid.water_tiles.contains(&ntile)
                    || world.grid.water_tiles.contains(&btile));
            if !is_lateral_water_edge {
                continue;
            }
            let rail_base = Vec3::new(tp[0], deck_y, tp[2]) + flank_off;
            let half_span = span_axis * (ts * 0.48);
            let truss_top_h = 0.88_f32;
            let b0 = rail_base - half_span;
            let b1 = rail_base + half_span;
            let t0 = b0 + Vec3::Y * truss_top_h;
            let t1 = b1 + Vec3::Y * truss_top_h;
            let t_mid = rail_base + Vec3::Y * truss_top_h;

            solid_batch.push_strut_3d(b0 + Vec3::Y * 0.06, b1 + Vec3::Y * 0.06, 0.055, curb_col);
            solid_batch.push_strut_3d(t0, t1, 0.052, truss_col);
            solid_batch.push_strut_3d(b0, t0, 0.048, truss_col);
            solid_batch.push_strut_3d(b1, t1, 0.048, truss_col);
            solid_batch.push_strut_3d(b0, t_mid, 0.036, truss_col);
            solid_batch.push_strut_3d(b1, t_mid, 0.036, truss_col);
        }
    }

    for span in &world.destroyed_bridges {
        let active_fp = if span.rotated {
            [span.footprint[1], span.footprint[0]]
        } else {
            span.footprint
        };
        let center_w = world
            .grid
            .footprint_center_world(span.tile_origin, active_fp);
        if cached_fow.world_pos_visibility(world, center_w) == TileVisibility::Unexplored {
            continue;
        }
        let span_axis = if active_fp[0] >= active_fp[1] {
            Vec3::X
        } else {
            Vec3::Z
        };
        let lat_axis = Vec3::new(-span_axis.z, 0.0, span_axis.x);
        let half_len = (active_fp[0].max(active_fp[1]).max(2) as f32) * ts * 0.46;
        let base_c = Vec3::new(center_w[0], deck_y - 0.12, center_w[2]);
        for end_sign in [-1.0_f32, 1.0_f32] {
            let stub_c = base_c + span_axis * (end_sign * half_len);
            solid_batch.push_oriented_box(
                stub_c,
                Vec3::new(0.48, 0.16, 0.48),
                Quat::IDENTITY,
                [0.24, 0.24, 0.26, 1.0],
            );
            for lat_sign in [-1.0_f32, 1.0_f32] {
                let gir_a = stub_c + lat_axis * (lat_sign * 0.55) + Vec3::Y * 0.18;
                let gir_b = gir_a - span_axis * (end_sign * 0.95) - Vec3::Y * 0.42;
                solid_batch.push_strut_3d(gir_a, gir_b, 0.05, [0.34, 0.30, 0.28, 1.0]);
            }
        }
    }

    let mut hut_tiles: BTreeSet<[i32; 2]> = world.bridge_repair_hut_tiles.clone();
    for ent in world.entities.values() {
        if let EntityKind::Building { building_id } = &ent.kind {
            if loaded
                .buildings
                .get(building_id)
                .is_some_and(|b| b.role == BuildingRole::BridgeRepairHut)
            {
                let t = world.grid.world_to_tile(ent.position);
                hut_tiles.insert(t);
            }
        }
    }

    for htile in hut_tiles {
        if cached_fow.tile_visibility(htile) == TileVisibility::Unexplored {
            continue;
        }
        let hp = world.grid.tile_to_world(htile);
        let gy = world.grid.sample_world_height(hp[0], hp[2]).max(0.05);
        let hut_pos = Vec3::new(hp[0], gy, hp[2]);

        let nearby_demolished = world.destroyed_bridges.iter().any(|span| {
            let dx = (span.tile_origin[0] - htile[0]).abs();
            let dz = (span.tile_origin[1] - htile[1]).abs();
            dx <= 16 && dz <= 16
        });

        solid_batch.push_oriented_box(
            hut_pos + Vec3::Y * 0.42,
            Vec3::new(0.46, 0.42, 0.46),
            Quat::IDENTITY,
            [0.44, 0.48, 0.54, 1.0],
        );
        solid_batch.push_oriented_box(
            hut_pos + Vec3::Y * 0.90,
            Vec3::new(0.54, 0.08, 0.54),
            Quat::IDENTITY,
            [0.26, 0.34, 0.46, 1.0],
        );
        solid_batch.push_strut_3d(
            hut_pos + Vec3::Y * 0.96,
            hut_pos + Vec3::Y * 1.85,
            0.045,
            truss_col,
        );

        let pulse = 0.72 + (elapsed_secs * 5.0).sin().abs() * 0.28;
        let status_rgba = if nearby_demolished {
            [1.0, 0.56 * pulse + 0.22, 0.12, 0.96]
        } else {
            [0.20, 0.96, 0.46, 0.92]
        };
        billboard_batch.push_billboard_rect(
            hut_pos + Vec3::Y * 2.08,
            cam_right,
            cam_up,
            0.18,
            0.18,
            status_rgba,
        );
        if nearby_demolished {
            ground_batch.push_ground_ring(
                hut_pos + Vec3::Y * 0.08,
                1.35 * pulse,
                0.11,
                18,
                [1.0, 0.68, 0.16, 0.82],
            );
        }
    }
}

/// Emits 3D selection brackets, invulnerability/repair rings, overhead health/shield/suppression/squad bars,
/// cover shields, pinned alerts, and transport/ammo/veterancy pips (`ColorblindPaletteMode`-aware).
#[allow(clippy::too_many_arguments)]
pub(super) fn push_entity_selection_and_overhead_bars(
    ground_batch: &mut DynamicQuadBatchBuilder,
    billboard_batch: &mut DynamicQuadBatchBuilder,
    solid_batch: &mut DynamicQuadBatchBuilder,
    world: &SimWorld,
    loaded: &LoadedMod,
    sim_ent: &SimEntity,
    pos: Vec3,
    base_y: f32,
    is_selected: bool,
    is_friendly: bool,
    local_pid: u8,
    attack_move_aiming: bool,
    cb_mode: ColorblindPaletteMode,
    strategic_blend: f32,
    elapsed: f32,
    cam_right: Vec3,
    cam_up: Vec3,
) {
    if is_selected {
        let (hw, hd, leg, thick) = match &sim_ent.kind {
            EntityKind::Unit { .. } => (0.94, 0.94, 0.36, 0.055),
            EntityKind::Building { .. } => {
                let fp = world.entity_footprint(sim_ent, loaded);
                let ts = world.grid.tile_size;
                (
                    (fp[0] as f32 * ts * 0.52).max(1.8),
                    (fp[1] as f32 * ts * 0.52).max(1.8),
                    0.68,
                    0.08,
                )
            }
        };
        let by = base_y + 0.06;
        let brk_rgba = if is_friendly {
            if cb_mode == ColorblindPaletteMode::Off {
                [0.95, 0.99, 1.0, 0.96]
            } else {
                let c = resolve_accessible_team_color(
                    sim_ent.player_id,
                    sim_ent.player_id == local_pid,
                    true,
                    cb_mode,
                );
                [
                    (c[0] * 0.45 + 0.55).clamp(0.0, 1.0),
                    (c[1] * 0.45 + 0.55).clamp(0.0, 1.0),
                    (c[2] * 0.45 + 0.55).clamp(0.0, 1.0),
                    0.96,
                ]
            }
        } else {
            let c = resolve_accessible_team_color(sim_ent.player_id, false, false, cb_mode);
            [c[0], c[1], c[2], 0.94]
        };
        for sx in [-1.0_f32, 1.0_f32] {
            for sz in [-1.0_f32, 1.0_f32] {
                let c = Vec3::new(pos.x + sx * hw, by, pos.z + sz * hd);
                let ex = Vec3::new(pos.x + sx * (hw - leg), by, pos.z + sz * hd);
                let ez = Vec3::new(pos.x + sx * hw, by, pos.z + sz * (hd - leg));
                ground_batch.push_ground_line(c, ex, thick, brk_rgba);
                ground_batch.push_ground_line(c, ez, thick, brk_rgba);
            }
        }

        if attack_move_aiming {
            if let Some(wid) = &sim_ent.primary_weapon {
                if let Some(wdef) = loaded.weapons.get(wid) {
                    ground_batch.push_ground_ring(
                        Vec3::new(pos.x, base_y + 0.06, pos.z),
                        wdef.range,
                        0.10,
                        36,
                        [1.0, 0.42, 0.20, 0.34],
                    );
                }
            }
        }
    }

    if sim_ent.is_invulnerable() {
        let is_bldg = matches!(sim_ent.kind, EntityKind::Building { .. });
        let r = if is_bldg { 3.0 } else { 1.38 };
        let col = if is_bldg {
            [0.20, 0.80, 1.0, 0.76]
        } else {
            [0.96, 0.18, 0.20, 0.80]
        };
        ground_batch.push_ground_ring(pos + Vec3::Y * 0.10, r, 0.14, 24, col);
    }
    if sim_ent.is_repairing {
        ground_batch.push_ground_ring(
            pos + Vec3::Y * 0.10,
            2.6,
            0.10,
            24,
            [0.20, 0.98, 0.46, 0.70],
        );
    }

    if matches!(sim_ent.kind, EntityKind::Building { .. }) && sim_ent.construction_progress < 0.999
    {
        let fp = world.entity_footprint(sim_ent, loaded);
        let ts_b = world.grid.tile_size.max(1.0);
        let scaf_hw = (fp[0] as f32 * ts_b * 0.50).max(1.3);
        let scaf_hd = (fp[1] as f32 * ts_b * 0.50).max(1.3);
        let full_h = (fp[0].max(fp[1]) as f32 * 1.15).clamp(2.8, 5.4);
        let team_rgb = crate::rendering::team_material::accessible_team_color_rgb_for_player(
            world,
            loaded,
            sim_ent.player_id,
            local_pid,
            cb_mode,
        );
        push_building_construction_scaffolding_and_crane(
            solid_batch,
            pos,
            scaf_hw,
            scaf_hd,
            full_h,
            sim_ent.construction_progress,
            elapsed,
            team_rgb,
        );
    }

    // Fade tactical overhead micro-bars smoothly once Strategic Icons take over (`> 210m`)
    if strategic_blend >= 0.98 {
        return;
    }

    let live_cover = match &sim_ent.kind {
        EntityKind::Unit { .. } => {
            if sim_ent.cover_level != CoverLevel::None {
                sim_ent.cover_level
            } else {
                rts_simulation::world::squads::compute_unit_cover_level(world, sim_ent, loaded)
            }
        }
        EntityKind::Building { .. } => CoverLevel::None,
    };

    let show_bar = is_selected
        || sim_ent.health < sim_ent.max_health
        || sim_ent.suppression > 0.05
        || sim_ent.squad_members_alive > 1
        || live_cover != CoverLevel::None
        || sim_ent.is_pinned;
    if show_bar {
        let ratio = (sim_ent.health / sim_ent.max_health.max(1.0)).clamp(0.0, 1.0);
        let (bar_y, half_w, half_h) = match &sim_ent.kind {
            EntityKind::Unit { .. } => (base_y + 1.98, 0.76, 0.065),
            EntityKind::Building { .. } => (base_y + 3.55, 1.42, 0.085),
        };
        let bar_center = Vec3::new(pos.x, bar_y, pos.z);
        billboard_batch.push_billboard_rect(
            bar_center,
            cam_right,
            cam_up,
            half_w + 0.04,
            half_h + 0.03,
            [0.05, 0.07, 0.10, 0.92],
        );
        if ratio > 0.005 {
            let hp_rgba = if cb_mode != ColorblindPaletteMode::Off {
                if ratio > 0.50 {
                    [0.18, 0.82, 1.0, 0.98]
                } else if ratio > 0.25 {
                    [1.0, 0.84, 0.18, 0.98]
                } else {
                    [1.0, 0.42, 0.12, 0.98]
                }
            } else if ratio > 0.50 {
                [0.18, 0.96, 0.34, 0.98]
            } else if ratio > 0.25 {
                [0.98, 0.82, 0.16, 0.98]
            } else {
                [0.98, 0.22, 0.16, 0.98]
            };
            let fill_hw = half_w * ratio;
            let fill_center = bar_center - cam_right * (half_w - fill_hw)
                + cam_right.cross(cam_up).normalize_or_zero() * 0.01;
            billboard_batch.push_billboard_rect(
                fill_center,
                cam_right,
                cam_up,
                fill_hw,
                half_h,
                hp_rgba,
            );
        }

        if sim_ent.is_invulnerable() || sim_ent.temporal_progress > 0.01 {
            let s_ratio = if sim_ent.is_invulnerable() {
                1.0
            } else {
                (1.0 - sim_ent.temporal_progress / sim_ent.max_health.max(1.0)).clamp(0.0, 1.0)
            };
            let s_center = bar_center + cam_up * (half_h * 2.4);
            let fill_hw = half_w * s_ratio;
            billboard_batch.push_billboard_rect(
                s_center - cam_right * (half_w - fill_hw),
                cam_right,
                cam_up,
                fill_hw,
                half_h * 0.65,
                [0.32, 0.88, 1.0, 0.95],
            );
        }

        if sim_ent.suppression > 0.05 {
            let sup_ratio = sim_ent.suppression.clamp(0.0, 1.0);
            let sup_center = bar_center - cam_up * (half_h * 2.3);
            let sup_col = if sup_ratio >= 0.5 {
                [1.0, 0.45, 0.14, 0.95]
            } else {
                [0.92, 0.76, 0.22, 0.90]
            };
            let fill_hw = half_w * sup_ratio;
            billboard_batch.push_billboard_rect(
                sup_center - cam_right * (half_w - fill_hw),
                cam_right,
                cam_up,
                fill_hw,
                half_h * 0.58,
                sup_col,
            );
        }

        if sim_ent.squad_members_alive > 1 {
            let count = sim_ent.squad_members_alive.min(10);
            let pip_spacing = (half_w * 2.0) / (count as f32).max(1.0);
            let pip_hw = (pip_spacing * 0.36).min(0.11);
            let row_center = bar_center + cam_up * (half_h * 2.6);
            let start = -((count as f32 - 1.0) * pip_spacing * 0.5);
            for i in 0..count {
                let p_pos = row_center + cam_right * (start + i as f32 * pip_spacing);
                billboard_batch.push_billboard_rect(
                    p_pos,
                    cam_right,
                    cam_up,
                    pip_hw,
                    0.045,
                    [0.38, 0.95, 1.0, 0.96],
                );
            }
        }

        if let Some((shield_rgba, fortified)) = cover_level_shield_rgba(live_cover) {
            let shield_pos = bar_center - cam_right * (half_w + 0.22) + cam_up * 0.03;
            billboard_batch.push_billboard_shield_pip(
                shield_pos,
                cam_right,
                cam_up,
                1.0,
                shield_rgba,
                fortified,
            );
        }

        if sim_ent.is_pinned {
            let pin_lift = if sim_ent.squad_members_alive > 1 {
                half_h * 5.8
            } else {
                half_h * 4.2
            };
            let pin_pulse = (elapsed * 7.5).sin().abs();
            billboard_batch.push_billboard_pinned_badge(
                bar_center + cam_up * pin_lift,
                cam_right,
                cam_up,
                1.0,
                pin_pulse,
            );
        }
    }

    let cap = world.carrier_capacity(sim_ent, loaded);
    if cap > 0 && (is_selected || !sim_ent.passenger_ids.is_empty()) {
        let pip_y = match &sim_ent.kind {
            EntityKind::Unit { .. } => base_y + 2.30,
            EntityKind::Building { .. } => base_y + 3.85,
        };
        let spacing = 0.26_f32;
        let start = -((cap.min(8) as f32 - 1.0) * spacing * 0.5);
        let row_c = Vec3::new(pos.x, pip_y, pos.z);
        for slot in 0..cap.min(8) {
            let filled = (slot as usize) < sim_ent.passenger_ids.len();
            let col = if filled {
                [0.24, 0.96, 1.0, 0.96]
            } else {
                [0.25, 0.30, 0.36, 0.78]
            };
            billboard_batch.push_billboard_rect(
                row_c + cam_right * (start + slot as f32 * spacing),
                cam_right,
                cam_up,
                0.09,
                0.055,
                col,
            );
        }
    }

    if sim_ent.max_ammo > 0 && is_selected {
        let spacing = 0.26_f32;
        let n = sim_ent.max_ammo.min(8);
        let start = -((n as f32 - 1.0) * spacing * 0.5);
        let row_c = Vec3::new(pos.x, base_y + 1.68, pos.z);
        for slot in 0..n {
            let col = if slot < sim_ent.current_ammo {
                [1.0, 0.74, 0.16, 0.96]
            } else {
                [0.28, 0.28, 0.32, 0.72]
            };
            billboard_batch.push_billboard_rect(
                row_c + cam_right * (start + slot as f32 * spacing),
                cam_right,
                cam_up,
                0.09,
                0.045,
                col,
            );
        }
    }

    if sim_ent.veterancy != VeterancyRank::Rookie {
        let badge_c = Vec3::new(pos.x, base_y + 2.25, pos.z) + cam_right * 0.88;
        billboard_batch.push_billboard_rect(
            badge_c,
            cam_right,
            cam_up,
            0.10,
            0.08,
            [1.0, 0.86, 0.16, 0.98],
        );
        if sim_ent.veterancy == VeterancyRank::Elite {
            billboard_batch.push_billboard_rect(
                badge_c + cam_up * 0.18,
                cam_right,
                cam_up,
                0.12,
                0.08,
                [1.0, 0.92, 0.28, 0.98],
            );
        }
    }
}
