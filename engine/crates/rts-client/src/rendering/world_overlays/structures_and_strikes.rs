use super::batch_builder::DynamicQuadBatchBuilder;
use super::tactical_hud3d::{push_bridges_and_repair_huts, push_tactical_ping_pillar_and_rings};
use crate::input::ClientInteractionState;
use crate::rendering::{CachedFogVisibility, ColorblindPaletteMode};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;
use rts_simulation::world::{
    EntityKind, GameMode, SimWorld, SupportAircraftPayload, TileVisibility,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn push_placement_zones_walls_aircraft_and_missiles(
    ground_batch: &mut DynamicQuadBatchBuilder,
    billboard_batch: &mut DynamicQuadBatchBuilder,
    solid_batch: &mut DynamicQuadBatchBuilder,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    cached_fow: &CachedFogVisibility,
    local_pid: u8,
    cb_mode: ColorblindPaletteMode,
    elapsed: f32,
    cam_right: Vec3,
    cam_up: Vec3,
) {
    if !interaction.studio_open {
        if let Some(power_id) = &interaction.active_support_power_id {
            let hov_pos = world.grid.tile_to_world(interaction.hovered_tile);
            let target_pt = Vec3::new(hov_pos[0], hov_pos[1].max(0.08) + 0.08, hov_pos[2]);
            let sp_rad = loaded
                .buildings
                .values()
                .flat_map(|b| &b.support_powers)
                .find(|sp| &sp.id == power_id)
                .map(|sp| sp.radius.max(3.5))
                .unwrap_or(6.0);
            ground_batch.push_ground_ring(target_pt, sp_rad, 0.14, 32, [1.0, 0.68, 0.16, 0.85]);
            ground_batch.push_ground_ring(target_pt, 1.05, 0.10, 20, [1.0, 0.82, 0.24, 0.90]);
        }

        if let Some(b_id) = &interaction.placing_building_id {
            if let Some(b_def) = loaded.buildings.get(b_id) {
                let ts = world.grid.tile_size;
                let hov = interaction.hovered_tile;
                let active_fp = if interaction.placement_rotated {
                    [b_def.footprint[1], b_def.footprint[0]]
                } else {
                    b_def.footprint
                };
                let within_radius = world.is_within_build_radius(local_pid, hov, active_fp);

                for dz in -3i32..=4i32 {
                    for dx in -3i32..=4i32 {
                        let t = [hov[0] + dx, hov[1] + dz];
                        if !world.grid.in_bounds(t) {
                            continue;
                        }
                        let walkable = world.grid.is_walkable(t);
                        let cp = world.grid.tile_to_world(t);
                        let col = if walkable {
                            [0.20, 0.92, 0.42, 0.16]
                        } else {
                            [0.95, 0.22, 0.22, 0.24]
                        };
                        ground_batch.push_ground_rect(
                            Vec3::new(cp[0], cp[1].max(0.02) + 0.04, cp[2]),
                            ts * 0.88,
                            ts * 0.88,
                            col,
                        );
                    }
                }

                for dz in 0..active_fp[1] as i32 {
                    for dx in 0..active_fp[0] as i32 {
                        let sub_t = [hov[0] + dx, hov[1] + dz];
                        let sub_ok = world.grid.in_bounds(sub_t)
                            && (b_def.role == BuildingRole::Bridge
                                || b_def.water_bound
                                || world.grid.is_walkable(sub_t))
                            && within_radius;
                        let sp = world.grid.tile_to_world(sub_t);
                        let col = if sub_ok {
                            [0.16, 1.0, 0.40, 0.56]
                        } else {
                            [1.0, 0.18, 0.18, 0.62]
                        };
                        ground_batch.push_ground_rect(
                            Vec3::new(sp[0], sp[1].max(0.02) + 0.07, sp[2]),
                            ts * 0.94,
                            ts * 0.94,
                            col,
                        );
                    }
                }

                if let Some(w_id) = &b_def.primary_weapon {
                    if let Some(w_def) = loaded.weapons.get(w_id) {
                        let center = world.grid.footprint_center_world(hov, active_fp);
                        ground_batch.push_ground_ring(
                            Vec3::new(center[0], center[1].max(0.04) + 0.08, center[2]),
                            w_def.range,
                            0.12,
                            36,
                            [1.0, 0.58, 0.18, 0.48],
                        );
                    }
                }
            }
        }
    }

    for zone in &world.radiation_zones {
        if !cached_fow.is_world_pos_visible(world, zone.position) {
            continue;
        }
        let z_pos = Vec3::new(
            zone.position[0],
            zone.position[1].max(0.04) + 0.08,
            zone.position[2],
        );
        let a = (zone.remaining_secs / 6.0).clamp(0.18, 0.72);
        let col = if zone.is_toxic_cloud {
            [0.72, 0.30, 0.96, a]
        } else {
            [0.24, 1.0, 0.20, a]
        };
        ground_batch.push_ground_ring(z_pos, zone.radius, 0.16, 24, col);
    }

    for ping in &world.active_pings {
        if ping.player_id != local_pid
            && (ping.player_id == 0 || world.are_hostile(local_pid, ping.player_id))
        {
            continue;
        }
        let team_rgb = crate::rendering::team_material::accessible_team_color_rgb_for_player(
            world,
            loaded,
            ping.player_id,
            local_pid,
            cb_mode,
        );
        push_tactical_ping_pillar_and_rings(
            ground_batch,
            billboard_batch,
            Vec3::from_array(ping.position),
            ping.remaining_secs,
            team_rgb,
            cam_right,
            cam_up,
        );
    }

    if world.game_mode == GameMode::KingOfTheHill {
        let cw = world
            .grid
            .tile_to_world([world.grid.width / 2, world.grid.height / 2]);
        ground_batch.push_ground_ring(
            Vec3::new(cw[0], cw[1].max(0.04) + 0.10, cw[2]),
            10.0,
            0.18,
            36,
            [1.0, 0.82, 0.18, 0.55],
        );
    }

    // NESW Auto-Connecting 3D Fortress Walls & Animated Gates (`openness_01`)
    let ts = world.grid.tile_size.max(1.0);
    for &wtile in &world.grid.wall_tiles {
        if cached_fow.tile_visibility(wtile) == TileVisibility::Unexplored {
            continue;
        }
        let tp = world.grid.tile_to_world(wtile);
        let gy = world.grid.sample_world_height(tp[0], tp[2]);
        let mask = world.grid.wall_neighbor_mask(wtile);

        if let Some(gate_state) = world.gate_states.get(&wtile) {
            let openness = gate_state.openness_01.clamp(0.0, 1.0);
            let spans_ns = (mask & 0b0101) != 0 && (mask & 0b1010) == 0;
            let flank_axis = if spans_ns { Vec3::Z } else { Vec3::X };
            let leaf_half = if spans_ns {
                Vec3::new(0.18, 0.62, ts * 0.34)
            } else {
                Vec3::new(ts * 0.34, 0.62, 0.18)
            };
            let pillar_col = [0.46, 0.50, 0.56, 1.0];
            let beacon_col = if openness > 0.15 {
                [0.24, 0.98, 0.48, 1.0]
            } else {
                [0.98, 0.62, 0.16, 1.0]
            };
            for side in [-1.0_f32, 1.0_f32] {
                let p_base = Vec3::new(tp[0], gy + 0.78, tp[2]) + flank_axis * (side * ts * 0.42);
                solid_batch.push_oriented_box(
                    p_base,
                    Vec3::new(0.24, 0.78, 0.24),
                    Quat::IDENTITY,
                    pillar_col,
                );
                solid_batch.push_oriented_box(
                    p_base + Vec3::Y * 0.84,
                    Vec3::new(0.11, 0.08, 0.11),
                    Quat::IDENTITY,
                    beacon_col,
                );
            }
            solid_batch.push_oriented_box(
                Vec3::new(tp[0], gy + 0.05, tp[2]),
                if spans_ns {
                    Vec3::new(0.32, 0.05, ts * 0.48)
                } else {
                    Vec3::new(ts * 0.48, 0.05, 0.32)
                },
                Quat::IDENTITY,
                [0.28, 0.31, 0.35, 1.0],
            );
            if openness < 0.98 {
                let visible_h = leaf_half.y * (1.0 - openness * 0.92);
                let leaf_center = Vec3::new(tp[0], gy + visible_h + 0.06, tp[2]);
                solid_batch.push_oriented_box(
                    leaf_center,
                    Vec3::new(leaf_half.x, visible_h.max(0.04), leaf_half.z),
                    Quat::IDENTITY,
                    [0.62, 0.58, 0.44, 1.0],
                );
            }
            continue;
        }

        let wall_col = [0.64, 0.66, 0.70, 1.0];
        let cap_col = [0.42, 0.45, 0.50, 1.0];
        solid_batch.push_oriented_box(
            Vec3::new(tp[0], gy + 0.70, tp[2]),
            Vec3::new(0.26, 0.70, 0.26),
            Quat::IDENTITY,
            wall_col,
        );
        solid_batch.push_oriented_box(
            Vec3::new(tp[0], gy + 1.44, tp[2]),
            Vec3::new(0.30, 0.06, 0.30),
            Quat::IDENTITY,
            cap_col,
        );

        let arm_reach = ts * 0.26;
        let arm_offset = ts * 0.25;
        let dirs: [(u8, [i32; 2], Vec3, Vec3); 4] = [
            (
                1,
                [wtile[0], wtile[1] - 1],
                Vec3::new(0.0, 0.0, -arm_offset),
                Vec3::new(0.18, 0.60, arm_reach),
            ),
            (
                2,
                [wtile[0] + 1, wtile[1]],
                Vec3::new(arm_offset, 0.0, 0.0),
                Vec3::new(arm_reach, 0.60, 0.18),
            ),
            (
                4,
                [wtile[0], wtile[1] + 1],
                Vec3::new(0.0, 0.0, arm_offset),
                Vec3::new(0.18, 0.60, arm_reach),
            ),
            (
                8,
                [wtile[0] - 1, wtile[1]],
                Vec3::new(-arm_offset, 0.0, 0.0),
                Vec3::new(arm_reach, 0.60, 0.18),
            ),
        ];
        for (bit, ntile, delta_xz, half_ext) in dirs {
            if (mask & bit) != 0 {
                let np = world.grid.tile_to_world(ntile);
                let n_gy = world.grid.sample_world_height(np[0], np[2]);
                let seg_y = (gy * 0.65 + n_gy * 0.35) + 0.60;
                solid_batch.push_oriented_box(
                    Vec3::new(tp[0] + delta_xz.x, seg_y, tp[2] + delta_xz.z),
                    half_ext,
                    Quat::IDENTITY,
                    wall_col,
                );
            }
        }
    }

    push_bridges_and_repair_huts(
        solid_batch,
        ground_batch,
        billboard_batch,
        world,
        loaded,
        cached_fow,
        elapsed,
        cam_right,
        cam_up,
    );

    // Physical `active_support_aircraft` sorties
    for sortie in &world.active_support_aircraft {
        let friendly = sortie.owner_player_id == local_pid
            || (sortie.owner_player_id != 0
                && !world.are_hostile(local_pid, sortie.owner_player_id));
        if !sortie.dropped
            && (friendly || cached_fow.is_world_pos_visible(world, sortie.target_pos))
        {
            let ty = world
                .grid
                .sample_world_height(sortie.target_pos[0], sortie.target_pos[2])
                .max(0.04)
                + 0.10;
            let tpt = Vec3::new(sortie.target_pos[0], ty, sortie.target_pos[2]);
            let (rad, col) = match &sortie.payload {
                SupportAircraftPayload::Paradrop { .. } => (2.8, [0.28, 0.90, 1.0, 0.78]),
                SupportAircraftPayload::Airstrike { radius, .. } => {
                    (radius.max(3.2), [1.0, 0.42, 0.18, 0.84])
                }
            };
            let pulse = 0.85 + (elapsed * 4.5).sin().abs() * 0.25;
            ground_batch.push_ground_ring(tpt, rad * pulse, 0.14, 24, col);
        }

        if let Some(plane_ent) = world.entities.get(&sortie.entity_id) {
            let has_catalog_glb = match &plane_ent.kind {
                EntityKind::Unit { unit_id } => loaded.units.contains_key(unit_id),
                EntityKind::Building { building_id } => loaded.buildings.contains_key(building_id),
            };
            let plane_vis = friendly || cached_fow.is_entity_visible(world, loaded, plane_ent);
            if !has_catalog_glb && plane_vis {
                let ppos = Vec3::from_array(plane_ent.position);
                let prot = Quat::from_rotation_y(plane_ent.facing_radians);
                let hull_col = match &sortie.payload {
                    SupportAircraftPayload::Paradrop { .. } => [0.38, 0.56, 0.68, 1.0],
                    SupportAircraftPayload::Airstrike { .. } => [0.30, 0.34, 0.40, 1.0],
                };
                solid_batch.push_oriented_box(ppos, Vec3::new(0.42, 0.34, 1.95), prot, hull_col);
                solid_batch.push_oriented_box(
                    ppos + prot * Vec3::new(0.0, 0.06, -0.10),
                    Vec3::new(2.35, 0.08, 0.46),
                    prot,
                    hull_col,
                );
                solid_batch.push_oriented_box(
                    ppos + prot * Vec3::new(0.0, 0.48, -1.55),
                    Vec3::new(0.08, 0.42, 0.36),
                    prot,
                    [0.72, 0.24, 0.20, 1.0],
                );
            }
        }
    }

    // Ballistic `pending_missile_strikes` nuclear warheads + pulsing ground hazard reticle
    for strike in &world.pending_missile_strikes {
        let friendly = strike.owner_player_id == local_pid
            || (strike.owner_player_id != 0
                && !world.are_hostile(local_pid, strike.owner_player_id));
        if !(friendly || cached_fow.is_world_pos_visible(world, strike.target_pos)) {
            continue;
        }
        let gy = world
            .grid
            .sample_world_height(strike.target_pos[0], strike.target_pos[2])
            .max(0.04);
        let impact_pt = Vec3::new(strike.target_pos[0], gy + 0.12, strike.target_pos[2]);
        let prog = (1.0 - strike.remaining_secs / strike.total_secs.max(0.25)).clamp(0.0, 1.0);
        let warn_alpha = (0.55 + (elapsed * 9.0).sin().abs() * 0.40).clamp(0.4, 0.98);
        ground_batch.push_ground_ring(
            impact_pt,
            strike.radius.max(4.0),
            0.22,
            36,
            [1.0, 0.16, 0.12, warn_alpha],
        );
        ground_batch.push_ground_ring(
            impact_pt,
            (strike.radius.max(4.0) * (1.0 - prog)).max(0.65),
            0.16,
            24,
            [1.0, 0.78, 0.14, 0.92],
        );

        let descend_t = ((prog - 0.15) / 0.85).clamp(0.0, 1.0);
        let alt = (1.0 - descend_t * descend_t) * 44.0 + 0.8;
        let lateral_drift = (1.0 - descend_t) * 14.0;
        let warhead_pos = Vec3::new(
            strike.target_pos[0] - lateral_drift * 0.45,
            gy + alt,
            strike.target_pos[2] - lateral_drift,
        );
        let dive_dir = (impact_pt - warhead_pos).normalize_or_zero();
        let warhead_rot = if dive_dir.length_squared() > 0.1 {
            Quat::from_rotation_arc(Vec3::Z, dive_dir)
        } else {
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
        };
        solid_batch.push_oriented_box(
            warhead_pos,
            Vec3::new(0.28, 0.28, 1.45),
            warhead_rot,
            [0.82, 0.84, 0.88, 1.0],
        );
        solid_batch.push_oriented_box(
            warhead_pos + warhead_rot * Vec3::new(0.0, 0.0, 1.25),
            Vec3::new(0.22, 0.22, 0.42),
            warhead_rot,
            [0.92, 0.18, 0.14, 1.0],
        );
        billboard_batch.push_billboard_rect(
            warhead_pos - dive_dir * 1.75,
            cam_right,
            cam_up,
            0.65,
            0.65,
            [1.0, 0.72, 0.18, 0.94],
        );
    }
}
