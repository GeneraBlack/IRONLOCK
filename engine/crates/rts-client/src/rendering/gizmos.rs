use super::gizmos_world::draw_studio_and_dev_world_gizmos;
use super::*;
use crate::input::{ClientInteractionState, EnvironmentPreset};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{BuildingRole, UnitRole};
use rts_simulation::world::EntityKind;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw_rts_gizmos(
    time: Res<Time>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    interaction: Res<ClientInteractionState>,
    cached_fow: Res<CachedFogVisibility>,
    studio_debug_cfg: Option<Res<StudioDebugOverlayConfig>>,
    socket_reg: Option<Res<SocketRegistry>>,
    visual_q: Query<&SimVisualEntity>,
    mut ambient_light: ResMut<AmbientLight>,
    mut dir_lights: Query<
        (&MainDirectionalLight, &mut DirectionalLight, &mut Transform),
        (
            Without<PreviewStageLight>,
            Without<PreviewStageEntity>,
            Without<SimVisualEntity>,
            Without<BuildingPointLight>,
            Without<crate::camera::RtsCameraRig>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
        ),
    >,
    mut point_lights: Query<
        (&mut PointLight, &mut Visibility),
        (With<BuildingPointLight>, Without<PreviewStageLight>),
    >,
    mut gizmos: Gizmos,
) {
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let elapsed = time.elapsed_secs();
    let map_center = Vec3::new(
        world.grid.width.max(1) as f32 * world.grid.tile_size * 0.5,
        0.0,
        world.grid.height.max(1) as f32 * world.grid.tile_size * 0.5,
    );

    // 1. Always sync dynamic Weather / Time-of-Day PBR Lighting Preset (`EnvironmentPreset`)
    //    including dynamic sun azimuth, elevation, color, illuminance, and balanced sky fill!
    let (amb_col, amb_bright, sun_col, sun_illum, sun_azim, sun_elev, fill_col, fill_illum) =
        match interaction.environment_preset {
            EnvironmentPreset::Day => (
                Color::srgb(0.74, 0.82, 0.96),
                175.0,
                Color::srgb(1.0, 0.96, 0.88),
                8_200.0,
                0.68_f32,
                1.02_f32,
                Color::srgb(0.58, 0.74, 1.0),
                480.0,
            ),
            EnvironmentPreset::Dusk => (
                Color::srgb(0.88, 0.56, 0.42),
                135.0,
                Color::srgb(1.0, 0.58, 0.28),
                5_200.0,
                2.35_f32,
                0.36_f32,
                Color::srgb(0.52, 0.44, 0.78),
                360.0,
            ),
            EnvironmentPreset::Night => (
                Color::srgb(0.24, 0.36, 0.68),
                95.0,
                Color::srgb(0.42, 0.60, 0.98),
                2_100.0,
                -1.15_f32,
                0.84_f32,
                Color::srgb(0.20, 0.30, 0.56),
                220.0,
            ),
            EnvironmentPreset::IonStorm => {
                let lightning_flash: f32 =
                    if ((elapsed * 6.5).sin() > 0.92) && ((elapsed * 19.0).cos() > 0.4) {
                        2.35
                    } else {
                        1.0
                    };
                (
                    Color::srgb(0.55, 0.42, 0.85),
                    130.0 * lightning_flash,
                    Color::srgb(0.74, 0.52, 1.0),
                    4_100.0 * lightning_flash,
                    1.45_f32 + (elapsed * 0.14).sin() * 0.18,
                    0.68_f32,
                    Color::srgb(0.46, 0.32, 0.76),
                    340.0 * lightning_flash.min(1.4),
                )
            }
        };
    ambient_light.color = amb_col;
    ambient_light.brightness = amb_bright;

    let sun_dist = 90.0_f32;
    let sun_horiz = sun_dist * sun_elev.cos();
    let sun_pos = map_center
        + Vec3::new(
            sun_azim.cos() * sun_horiz,
            sun_dist * sun_elev.sin(),
            sun_azim.sin() * sun_horiz,
        );
    let fill_azim = sun_azim + std::f32::consts::PI * 0.82;
    let fill_pos = map_center + Vec3::new(fill_azim.cos() * 64.0, 52.0, fill_azim.sin() * 64.0);

    for (role, mut dl, mut tf) in &mut dir_lights {
        if role.is_primary_sun {
            dl.color = sun_col;
            dl.illuminance = sun_illum;
            *tf = Transform::from_translation(sun_pos).looking_at(map_center, Vec3::Y);
        } else {
            dl.color = fill_col;
            dl.illuminance = fill_illum;
            *tf = Transform::from_translation(fill_pos).looking_at(map_center, Vec3::Y);
        }
    }

    // Sync Dynamic 3D PBR PointLights on buildings for Dusk, Night & IonStorm illumination (culled in Daytime)
    let (pt_col, pt_intensity, pt_vis) = match interaction.environment_preset {
        EnvironmentPreset::Day => (Color::srgb(1.0, 0.85, 0.50), 0.0, Visibility::Hidden),
        EnvironmentPreset::Dusk => (
            Color::srgb(1.0, 0.78, 0.38),
            65_000.0,
            Visibility::Inherited,
        ),
        EnvironmentPreset::Night => (
            Color::srgb(1.0, 0.86, 0.48),
            135_000.0,
            Visibility::Inherited,
        ),
        EnvironmentPreset::IonStorm => (
            Color::srgb(0.68, 0.45, 1.0),
            95_000.0,
            Visibility::Inherited,
        ),
    };
    for (mut pl, mut vis) in &mut point_lights {
        pl.color = pt_col;
        pl.intensity = pt_intensity;
        *vis = pt_vis;
    }

    // 2. STRICT PLAYER-MODE GATE (Task 163):
    // All Player-Mode 3D world UI (selection brackets, overhead HP/shield/suppression/squad bars,
    // order/rally ribbons, placement grids, projectiles, and 16 `.glb` props) is GPU-rendered in
    // `world_overlays.rs`. CPU debug `Gizmos` are strictly reserved for `dev_mode_enabled || studio_open`
    // (and disabled in active multiplayer matches for fairplay).
    let allow_debug_gizmos = (interaction.dev_mode_enabled || interaction.studio_open)
        && (!interaction.is_multiplayer || world.winner_player_id.is_some());
    if !allow_debug_gizmos {
        return;
    }

    let mut entities_with_3d_turret = std::collections::BTreeSet::new();
    for vis in &visual_q {
        if vis.has_3d_turret_node {
            entities_with_3d_turret.insert(vis.sim_id);
        }
    }

    let ts = world.grid.tile_size;
    let df = &interaction.debug_gizmo_flags;

    if interaction.dev_mode_enabled {
        for sim_ent in world.entities.values() {
            if sim_ent.health <= 0.0
                || sim_ent.transported_by.is_some()
                || !cached_fow.is_entity_visible(world, loaded, sim_ent)
            {
                continue;
            }
            let base_y = sim_ent.position[1].max(0.12);
            let pos = Vec3::new(sim_ent.position[0], base_y, sim_ent.position[2]);

            // Team-color identification ring in Developer Mode
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

            // Sub-object turret debug line when model has no 3D Turret child node
            if !entities_with_3d_turret.contains(&sim_ent.id) {
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
                                gizmos.line(
                                    turret_pivot,
                                    barrel_tip,
                                    Color::srgba(0.75, 0.82, 0.90, 0.85),
                                );
                            }
                        }
                    }
                    EntityKind::Building { building_id } => {
                        if sim_ent.construction_progress >= 1.0 {
                            if let Some(b_def) = loaded.buildings.get(building_id) {
                                if b_def.role == BuildingRole::Defense
                                    && sim_ent.primary_weapon.is_some()
                                {
                                    let t_dir = Vec3::new(
                                        sim_ent.turret_facing_radians.sin(),
                                        0.0,
                                        sim_ent.turret_facing_radians.cos(),
                                    );
                                    let turret_pivot = pos + Vec3::Y * 1.35;
                                    let barrel_tip = turret_pivot + t_dir * 1.40;
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
            }
        }
    }

    // Standard F12 Debug Gizmo Flags
    if df.any_active() {
        for sim_ent in world.entities.values() {
            if sim_ent.health <= 0.0
                || sim_ent.transported_by.is_some()
                || !cached_fow.is_entity_visible(world, loaded, sim_ent)
            {
                continue;
            }
            let pos = Vec3::new(
                sim_ent.position[0],
                sim_ent.position[1].max(0.10) + 0.12,
                sim_ent.position[2],
            );
            if df.show_paths && !sim_ent.path_waypoints.is_empty() {
                let mut prev = pos;
                for wp in &sim_ent.path_waypoints {
                    let next_p = Vec3::new(wp[0], wp[1].max(0.10) + 0.15, wp[2]);
                    gizmos.line(prev, next_p, Color::srgba(0.20, 1.0, 0.45, 0.90));
                    prev = next_p;
                }
            }
            if df.show_weapon_ranges {
                if let Some(wid) = &sim_ent.primary_weapon {
                    if let Some(wdef) = loaded.weapons.get(wid) {
                        gizmos.circle(
                            Isometry3d::new(
                                pos,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
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
        if df.show_blocked_tiles {
            for &tile in &world.grid.blocked_tiles {
                let wp = world.grid.tile_to_world(tile);
                gizmos.cuboid(
                    Transform::from_xyz(wp[0], wp[1].max(0.0) + 0.15, wp[2]).with_scale(Vec3::new(
                        ts * 0.85,
                        0.3,
                        ts * 0.85,
                    )),
                    Color::srgba(1.0, 0.18, 0.18, 0.55),
                );
            }
        }
    }

    if df.show_trigger_zones || interaction.studio_open {
        for tz in &world.trigger_zones {
            let min_w = world.grid.tile_to_world(tz.min);
            let max_w = world.grid.tile_to_world(tz.max);
            let cx = (min_w[0] + max_w[0]) * 0.5;
            let cz = (min_w[2] + max_w[2]) * 0.5;
            let sx = (max_w[0] - min_w[0]).abs().max(ts);
            let sz = (max_w[2] - min_w[2]).abs().max(ts);
            gizmos.cuboid(
                Transform::from_xyz(cx, 0.6, cz).with_scale(Vec3::new(sx, 1.2, sz)),
                Color::srgba(0.95, 0.30, 1.0, 0.72),
            );
        }
    }

    // 3. Creator Workbench Studio 3D Debug Visualizers (`StudioDebugOverlayConfig`)
    draw_studio_and_dev_world_gizmos(
        &mut gizmos,
        world,
        loaded,
        &interaction,
        studio_debug_cfg.as_deref(),
        socket_reg.as_deref(),
    );
}
