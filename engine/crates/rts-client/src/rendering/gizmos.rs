use super::gizmos_world::{draw_selected_entity_gizmos, draw_world_and_placement_gizmos};
use super::*;
use crate::input::{ClientInteractionState, EnvironmentPreset};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{BuildingRole, UnitRole};
use rts_simulation::world::EntityKind;

#[allow(clippy::too_many_arguments)]
pub fn draw_rts_gizmos(
    time: Res<Time>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    interaction: Res<ClientInteractionState>,
    visual_q: Query<&SimVisualEntity>,
    mut ambient_light: ResMut<AmbientLight>,
    mut dir_lights: Query<&mut DirectionalLight>,
    mut point_lights: Query<&mut PointLight>,
    mut gizmos: Gizmos,
) {
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let local_pid = if interaction.observer_pov_player_id > 0 {
        interaction.observer_pov_player_id
    } else {
        interaction.local_player_id
    };
    let elapsed = time.elapsed_secs();

    // Sync dynamic Weather / Time-of-Day Lighting Preset (`EnvironmentPreset`)
    let (amb_col, amb_bright, sun_col, sun_illum) = match interaction.environment_preset {
        EnvironmentPreset::Day => (
            Color::srgb(0.74, 0.82, 0.96),
            200.0,
            Color::srgb(1.0, 0.95, 0.86),
            6_200.0,
        ),
        EnvironmentPreset::Dusk => (
            Color::srgb(0.88, 0.58, 0.45),
            165.0,
            Color::srgb(1.0, 0.62, 0.32),
            4_800.0,
        ),
        EnvironmentPreset::Night => (
            Color::srgb(0.28, 0.40, 0.72),
            115.0,
            Color::srgb(0.45, 0.62, 0.98),
            2_600.0,
        ),
        EnvironmentPreset::IonStorm => {
            let lightning_flash = if ((elapsed * 6.5).sin() > 0.92) && ((elapsed * 19.0).cos() > 0.4)
            {
                2.2
            } else {
                1.0
            };
            (
                Color::srgb(0.55, 0.42, 0.85),
                150.0 * lightning_flash,
                Color::srgb(0.72, 0.50, 1.0),
                3_800.0 * lightning_flash,
            )
        }
    };
    ambient_light.color = amb_col;
    ambient_light.brightness = amb_bright;
    for mut dl in &mut dir_lights {
        if dl.shadows_enabled {
            dl.color = sun_col;
            dl.illuminance = sun_illum;
        }
    }

    // Sync Dynamic 3D PBR PointLights on buildings for Dusk, Night & IonStorm illumination
    let (pt_col, pt_intensity) = match interaction.environment_preset {
        EnvironmentPreset::Day => (Color::srgb(1.0, 0.85, 0.50), 0.0),
        EnvironmentPreset::Dusk => (Color::srgb(1.0, 0.78, 0.38), 65_000.0),
        EnvironmentPreset::Night => (Color::srgb(1.0, 0.86, 0.48), 135_000.0),
        EnvironmentPreset::IonStorm => (Color::srgb(0.68, 0.45, 1.0), 95_000.0),
    };
    for mut pl in &mut point_lights {
        pl.color = pt_col;
        pl.intensity = pt_intensity;
    }

    let fow_vis_map = if interaction.fog_of_war_enabled {
        Some(world.compute_visibility_map(local_pid, loaded))
    } else {
        None
    };

    // Draw multi-stage 3D construction & deconstruction scaffolding, laser scanner ring, and rotating crane arm
    let ts = world.grid.tile_size;
    for vis in &visual_q {
        let state_prog = if let Some(dec_p) = vis.deconstruct_progress {
            Some((dec_p.clamp(0.03, 1.0), true))
        } else if vis.spawn_progress < 0.99 {
            Some((vis.spawn_progress.clamp(0.04, 1.0), false))
        } else {
            None
        };

        if let Some((prog, is_deconstruct)) = state_prog {
            if let Some(fp) = vis.building_footprint {
                let w = fp[0] as f32 * ts;
                let d = fp[1] as f32 * ts;
                let cx = vis.world_pos.x;
                let cz = vis.world_pos.z;
                let max_h = 2.65_f32;
                let active_y = 0.12 + prog * (max_h - 0.18);

                let frame_color = if is_deconstruct {
                    Color::srgba(1.0, 0.52, 0.16, 0.85)
                } else {
                    Color::srgba(1.0, 0.84, 0.22, 0.80)
                };
                let beam_color = if is_deconstruct {
                    Color::srgba(1.0, 0.72, 0.25, 0.95)
                } else {
                    Color::srgba(0.28, 0.95, 1.0, 0.95)
                };

                // 1. Outer 3D Scaffolding Cage
                gizmos.cuboid(
                    Transform::from_xyz(cx, max_h * 0.5, cz)
                        .with_scale(Vec3::new(w + 0.28, max_h, d + 0.28)),
                    frame_color,
                );

                // 2. Active Horizontal Laser Scanner / Welder Tier Plane moving with construction progress
                gizmos.cuboid(
                    Transform::from_xyz(cx, active_y, cz)
                        .with_scale(Vec3::new(w + 0.36, 0.08, d + 0.36)),
                    beam_color,
                );

                // 3. Four Diagonal Corner Cross-Braces from foundation to active build tier
                let hw = (w + 0.24) * 0.5;
                let hd = (d + 0.24) * 0.5;
                for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let c_bot = Vec3::new(cx + sx * hw, 0.06, cz + sz * hd);
                    let c_mid = Vec3::new(cx - sx * hw * 0.35, active_y, cz - sz * hd * 0.35);
                    gizmos.line(c_bot, c_mid, frame_color);
                }

                // 4. Rotating Overhead 3D Crane Boom & Vertical Welding Laser Beam
                let crane_angle =
                    elapsed * (if is_deconstruct { -2.8 } else { 2.4 }) + (vis.sim_id as f32);
                let boom_radius = (w.min(d) * 0.38).max(0.8);
                let crane_pivot = Vec3::new(cx - hw * 0.85, max_h + 0.35, cz - hd * 0.85);
                let crane_tip = Vec3::new(
                    cx + crane_angle.cos() * boom_radius,
                    max_h + 0.35,
                    cz + crane_angle.sin() * boom_radius,
                );
                let weld_target = Vec3::new(crane_tip.x, active_y, crane_tip.z);
                // Vertical mast + horizontal boom + welding laser
                gizmos.line(
                    Vec3::new(crane_pivot.x, 0.05, crane_pivot.z),
                    crane_pivot,
                    frame_color,
                );
                gizmos.line(crane_pivot, crane_tip, frame_color);
                gizmos.line(crane_tip, weld_target, beam_color);
            }
        }
    }

    // Draw selection rings, sub-object turrets/radar dishes, passenger pips, veterancy chevrons, and 3D health bars
    for sim_ent in world.entities.values() {
        if sim_ent.transported_by.is_some() {
            continue;
        }
        if let Some(vis_map) = &fow_vis_map {
            if sim_ent.player_id != local_pid {
                let tile = world.grid.world_to_tile(sim_ent.position);
                let tv = world.tile_visibility_in_map(vis_map, tile);
                let visible = match sim_ent.kind {
                    EntityKind::Unit { .. } => tv == TileVisibility::Visible,
                    EntityKind::Building { .. } => tv != TileVisibility::Unexplored,
                };
                if !visible {
                    continue;
                }
            }
        }
        let base_y = sim_ent.position[1].max(0.12);
        let pos = Vec3::new(sim_ent.position[0], base_y, sim_ent.position[2]);
        let is_selected = interaction.selected_ids.contains(&sim_ent.id);

        // Sub-Object 3D Animations: Articulated Turret Barrel with Recoil & Rotating Tech Center Radar Dish
        match &sim_ent.kind {
            EntityKind::Unit { unit_id } => {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    if u_def.role == UnitRole::Vehicle && sim_ent.primary_weapon.is_some() {
                        let t_dir = Vec3::new(
                            sim_ent.turret_facing_radians.sin(),
                            0.0,
                            sim_ent.turret_facing_radians.cos(),
                        );
                        let recoil_pull = sim_ent.recoil_timer * 1.1;
                        let turret_pivot = pos + Vec3::Y * 1.05;
                        let barrel_tip =
                            turret_pivot + t_dir * (1.55 - recoil_pull).max(0.75);
                        let barrel_col = if sim_ent.recoil_timer > 0.05 {
                            Color::srgb(1.0, 0.88, 0.30)
                        } else {
                            Color::srgba(0.75, 0.82, 0.90, 0.85)
                        };
                        gizmos.line(turret_pivot, barrel_tip, barrel_col);
                    }
                }
            }
            EntityKind::Building { building_id } => {
                if sim_ent.construction_progress >= 1.0 {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if b_def.role == BuildingRole::TechCenter {
                            // Rotating 3D Radar Dish on Tech Center roof
                            let dish_pivot = pos + Vec3::Y * 2.55;
                            let r_dir = Vec3::new(
                                sim_ent.turret_facing_radians.sin(),
                                0.25,
                                sim_ent.turret_facing_radians.cos(),
                            )
                            .normalize_or_zero();
                            let dish_col = Color::srgba(0.35, 0.92, 1.0, 0.88);
                            gizmos.line(dish_pivot, dish_pivot + r_dir * 1.15, dish_col);
                            gizmos.circle(
                                Isometry3d::new(
                                    dish_pivot + r_dir * 0.85,
                                    Quat::from_rotation_y(sim_ent.turret_facing_radians),
                                ),
                                0.55,
                                dish_col,
                            );
                        } else if b_def.role == BuildingRole::Defense
                            && sim_ent.primary_weapon.is_some()
                        {
                            let t_dir = Vec3::new(
                                sim_ent.turret_facing_radians.sin(),
                                0.0,
                                sim_ent.turret_facing_radians.cos(),
                            );
                            let recoil_pull = sim_ent.recoil_timer * 0.9;
                            let turret_pivot = pos + Vec3::Y * 1.35;
                            let barrel_tip =
                                turret_pivot + t_dir * (1.45 - recoil_pull).max(0.7);
                            gizmos.line(
                                turret_pivot,
                                barrel_tip,
                                Color::srgba(0.95, 0.78, 0.28, 0.85),
                            );
                        }
                    }
                }
            }
        }

        // Draw 3D Parachute Canopy & Suspension Cords when paratrooper is descending (`parachute_timer > 0.0`)
        if sim_ent.parachute_timer > 0.0 {
            let canopy_center = pos + Vec3::Y * 2.65;
            let silk_col = Color::srgba(0.95, 0.97, 1.0, 0.88);
            let cord_col = Color::srgba(0.80, 0.84, 0.90, 0.78);
            gizmos.sphere(Isometry3d::from_translation(canopy_center), 1.15, silk_col);
            let harness_pt = pos + Vec3::Y * 0.85;
            for (cx, cz) in [(-0.85, -0.85), (0.85, -0.85), (0.85, 0.85), (-0.85, 0.85)] {
                gizmos.line(
                    canopy_center + Vec3::new(cx, -0.25, cz),
                    harness_pt,
                    cord_col,
                );
            }
        }

        // Draw 3D Airfield Pad Docking Ring when aircraft is physically landed at an airfield
        if sim_ent.landed_at_airfield {
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.06,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.45,
                Color::srgba(0.25, 0.88, 1.0, 0.65),
            );
        }

        // Draw 3D Passenger / Garrison Slot Pips above Transports and Garrisoned Bunkers/Barracks
        let cap = world.carrier_capacity(sim_ent, loaded);
        if cap > 0 && (is_selected || !sim_ent.passenger_ids.is_empty()) {
            let pip_y = match &sim_ent.kind {
                EntityKind::Unit { .. } => base_y + 2.35,
                EntityKind::Building { .. } => base_y + 3.88,
            };
            let spacing = 0.34_f32;
            let start_x = pos.x - (cap as f32 - 1.0) * spacing * 0.5;
            for slot in 0..cap {
                let px = start_x + slot as f32 * spacing;
                let filled = (slot as usize) < sim_ent.passenger_ids.len();
                let pip_col = if filled {
                    Color::srgb(0.22, 0.96, 1.0)
                } else {
                    Color::srgba(0.35, 0.42, 0.50, 0.75)
                };
                gizmos.cuboid(
                    Transform::from_xyz(px, pip_y, pos.z).with_scale(Vec3::new(0.22, 0.16, 0.16)),
                    pip_col,
                );
            }
        }

        if let Some((ctrl_id, _)) = sim_ent.mind_controlled_by {
            if let Some(ctrl_ent) = world.entities.get(&ctrl_id) {
                let ctrl_pos = Vec3::new(
                    ctrl_ent.position[0],
                    ctrl_ent.position[1] + 1.2,
                    ctrl_ent.position[2],
                );
                gizmos.line(
                    ctrl_pos,
                    pos + Vec3::Y * 1.1,
                    Color::srgba(0.88, 0.25, 1.0, 0.78),
                );
            }
        }

        if sim_ent.is_repairing {
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.22,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                2.95,
                Color::srgba(0.20, 0.98, 0.45, 0.82),
            );
        }

        // Draw active generic StatusEffect / Buff visuals data-driven via `StatusVfxStyle` & `vfx_ring_color`
        for buff in &sim_ent.active_buffs {
            let [r, g, b, a] = buff.vfx_ring_color.unwrap_or([0.85, 0.85, 0.85, 0.90]);
            let ring_col = Color::srgba(r, g, b, a);
            match buff.vfx_style {
                rts_sdk::schema::StatusVfxStyle::None => {}
                rts_sdk::schema::StatusVfxStyle::Ring => {
                    let ring_r = if buff.invulnerable { 2.25 } else { 1.75 };
                    gizmos.circle(
                        Isometry3d::new(
                            pos + Vec3::Y * 0.28,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        ring_r,
                        ring_col,
                    );
                }
                rts_sdk::schema::StatusVfxStyle::ElectricArcs => {
                    gizmos.circle(
                        Isometry3d::new(
                            pos + Vec3::Y * 0.38,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        1.95,
                        ring_col,
                    );
                    let arc_angle = elapsed * 16.0 + sim_ent.id as f32;
                    let arc_a =
                        pos + Vec3::new(arc_angle.cos() * 1.15, 0.35, arc_angle.sin() * 1.15);
                    let arc_b = pos
                        + Vec3::new((-arc_angle).cos() * 0.85, 1.65, (-arc_angle).sin() * 0.85);
                    gizmos.line(arc_a, arc_b, Color::srgb(0.55, 0.96, 1.0));
                }
                rts_sdk::schema::StatusVfxStyle::ParasiteSparks => {
                    let pulse_r = 1.45 + (elapsed * 9.0).sin().abs() * 0.45;
                    gizmos.circle(
                        Isometry3d::new(
                            pos + Vec3::Y * 0.34,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        pulse_r,
                        ring_col,
                    );
                    let spark_ang = elapsed * 8.0 + sim_ent.id as f32;
                    let spark_pt =
                        pos + Vec3::new(spark_ang.cos() * 0.85, 0.95, spark_ang.sin() * 0.85);
                    gizmos.sphere(
                        Isometry3d::from_translation(spark_pt),
                        0.22,
                        Color::srgb(r, g, b),
                    );
                }
                rts_sdk::schema::StatusVfxStyle::CountdownBeacon => {
                    let flash = (elapsed * 14.0).sin().abs();
                    let c4_col = Color::srgba(r, (g + flash * 0.65).min(1.0), b, 0.96);
                    let beacon_pos = pos + Vec3::Y * 3.1;
                    gizmos.sphere(
                        Isometry3d::from_translation(beacon_pos),
                        0.36 + flash * 0.14,
                        c4_col,
                    );
                    gizmos.circle(
                        Isometry3d::new(
                            pos + Vec3::Y * 0.42,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        1.1 + (buff.remaining_secs / 3.5).clamp(0.0, 2.0) * 1.8,
                        c4_col,
                    );
                }
                rts_sdk::schema::StatusVfxStyle::TentacleGrapple => {
                    for h in [0.35_f32, 0.75, 1.15] {
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * h,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            1.45,
                            ring_col,
                        );
                    }
                }
                rts_sdk::schema::StatusVfxStyle::TemporalVortex => {
                    let frac =
                        (sim_ent.temporal_progress / sim_ent.max_health.max(1.0)).clamp(0.0, 1.0);
                    gizmos.circle(
                        Isometry3d::new(
                            pos + Vec3::Y * (0.45 + frac * 1.1),
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        1.35 * (1.0 - frac * 0.45),
                        ring_col,
                    );
                }
            }
        }

        // Draw Service Depot docking guide line when unit is heading to a Service Depot
        if let Some(depot_id) = sim_ent.service_depot_target_id {
            if let Some(depot_ent) = world.entities.get(&depot_id) {
                let d_pos = Vec3::new(
                    depot_ent.position[0],
                    depot_ent.position[1].max(0.18) + 0.5,
                    depot_ent.position[2],
                );
                gizmos.line(
                    pos + Vec3::Y * 0.4,
                    d_pos,
                    Color::srgba(0.20, 0.98, 0.52, 0.80),
                );
            }
        }

        // Ground team identification rings are shown in Engine Developer Mode [F12];
        // Player Game Mode uses authentic C&C/RA2 white corner selection brackets `[ ]` + top HP bar.
        if interaction.dev_mode_enabled {
            let team_col = player_team_color(sim_ent.player_id);
            let team_ring_r = match &sim_ent.kind {
                EntityKind::Unit { .. } => 1.05,
                EntityKind::Building { .. } => 2.45,
            };
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.05,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                team_ring_r,
                team_col,
            );
        }

        if let Some((_, rem_s, tot_s)) = &sim_ent.upgrading_to {
            let prog = (1.0 - (*rem_s / tot_s.max(0.1))).clamp(0.0, 1.0);
            let upg_col = Color::srgba(1.0, 0.85, 0.18, 0.95);
            let ring_y = pos.y + 0.35 + prog * 2.1;
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(pos.x, ring_y, pos.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                2.55,
                upg_col,
            );
            let spin = elapsed * 4.5 + sim_ent.id as f32;
            for k in 0..4 {
                let a = spin + k as f32 * std::f32::consts::FRAC_PI_2;
                let beam_x = pos.x + a.cos() * 2.2;
                let beam_z = pos.z + a.sin() * 2.2;
                gizmos.line(
                    Vec3::new(beam_x, pos.y + 0.1, beam_z),
                    Vec3::new(beam_x, ring_y, beam_z),
                    Color::srgba(0.28, 0.96, 1.0, 0.90),
                );
            }
        }

        if sim_ent.salvage_upgraded {
            let salv_col = Color::srgba(0.22, 0.95, 1.0, 0.92);
            gizmos.cuboid(
                Transform::from_xyz(pos.x + 0.58, base_y + 1.45, pos.z)
                    .with_rotation(Quat::from_rotation_y(elapsed * 2.2))
                    .with_scale(Vec3::splat(0.24)),
                salv_col,
            );
        }

        if sim_ent.burrowed {
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(pos.x, 0.14, pos.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.25 + (elapsed * 10.0).sin().abs() * 0.25,
                Color::srgba(0.68, 0.48, 0.24, 0.92),
            );
        }

        if sim_ent.permanent_cloak {
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.32,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.45,
                Color::srgba(0.35, 0.95, 0.92, 0.72),
            );
        }

        // Draw Gap Generator / Mobile Stealth Generator active cloak field dome
        let cloak_field_r = world.active_entity_cloak_field_radius(sim_ent, &mod_res.0);
        if cloak_field_r > 0.0 && (is_selected || sim_ent.player_id == local_pid) {
            let dome_col = Color::srgba(0.32, 0.65, 1.0, 0.35);
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(pos.x, 0.14, pos.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                cloak_field_r,
                dome_col,
            );
        }

        draw_selected_entity_gizmos(
            &mut gizmos,
            world,
            loaded,
            &interaction,
            sim_ent,
            pos,
            base_y,
            is_selected,
        );
    }
    draw_world_and_placement_gizmos(
        &mut gizmos,
        world,
        loaded,
        &interaction,
        local_pid,
        elapsed,
    );

    // 3D Debug Gizmo Layers (Task 137: paths, weapon ranges, vision/stealth, trigger zones, collision grid)
    let df = &interaction.debug_gizmo_flags;
    if df.any_active() {
        for sim_ent in world.entities.values() {
            if sim_ent.health <= 0.0 || sim_ent.transported_by.is_some() {
                continue;
            }
            let pos = Vec3::new(sim_ent.position[0], 0.22, sim_ent.position[2]);
            if df.show_paths && !sim_ent.path_waypoints.is_empty() {
                let mut prev = pos;
                for wp in &sim_ent.path_waypoints {
                    let next_p = Vec3::new(wp[0], 0.25, wp[2]);
                    gizmos.line(prev, next_p, Color::srgba(0.20, 1.0, 0.45, 0.90));
                    prev = next_p;
                }
            }
            if df.show_weapon_ranges {
                if let Some(wid) = &sim_ent.primary_weapon {
                    if let Some(wdef) = loaded.weapons.get(wid) {
                        gizmos.circle(
                            Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                            wdef.range,
                            Color::srgba(1.0, 0.25, 0.20, 0.55),
                        );
                        if wdef.min_range > 0.1 {
                            gizmos.circle(
                                Isometry3d::new(
                                    pos,
                                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                                ),
                                wdef.min_range,
                                Color::srgba(1.0, 0.78, 0.18, 0.55),
                            );
                        }
                    }
                }
            }
            if df.show_vision_and_stealth {
                let vis_r = match &sim_ent.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.vision_range)
                        .unwrap_or(10.0),
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b| b.vision_range)
                        .unwrap_or(12.0),
                };
                gizmos.circle(
                    Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    vis_r,
                    Color::srgba(0.25, 0.85, 1.0, 0.45),
                );
            }
        }
        if df.show_trigger_zones {
            for tz in &world.trigger_zones {
                let min_w = world.grid.tile_to_world(tz.min);
                let max_w = world.grid.tile_to_world(tz.max);
                let cx = (min_w[0] + max_w[0]) * 0.5;
                let cz = (min_w[2] + max_w[2]) * 0.5;
                let sx = (max_w[0] - min_w[0]).abs().max(ts);
                let sz = (max_w[2] - min_w[2]).abs().max(ts);
                gizmos.cuboid(
                    Transform::from_xyz(cx, 0.6, cz).with_scale(Vec3::new(sx, 1.2, sz)),
                    Color::srgba(0.95, 0.30, 1.0, 0.78),
                );
            }
        }
        if df.show_blocked_tiles {
            for &tile in &world.grid.blocked_tiles {
                let wp = world.grid.tile_to_world(tile);
                gizmos.cuboid(
                    Transform::from_xyz(wp[0], 0.15, wp[2])
                        .with_scale(Vec3::new(ts * 0.85, 0.3, ts * 0.85)),
                    Color::srgba(1.0, 0.18, 0.18, 0.55),
                );
            }
        }
    }
}
