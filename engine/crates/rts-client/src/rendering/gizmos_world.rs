use crate::input::{ClientInteractionState, EnvironmentPreset};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, CrateKind, PropKind};
use rts_simulation::world::{EntityKind, GameMode, SimEntity, SimWorld, VeterancyRank};

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_selected_entity_gizmos(
    gizmos: &mut Gizmos,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    sim_ent: &SimEntity,
    pos: Vec3,
    base_y: f32,
    is_selected: bool,
) {
    let local_pid = if interaction.observer_pov_player_id > 0 { interaction.observer_pov_player_id } else { interaction.local_player_id };
        if is_selected {
            // Draw Spy Infiltration target link when heading to infiltrate an enemy building
            if let Some(inf_id) = sim_ent.infiltrate_target_id {
                if let Some(inf_ent) = world.entities.get(&inf_id) {
                    let inf_pos = Vec3::new(
                        inf_ent.position[0],
                        inf_ent.position[1].max(0.16) + 0.65,
                        inf_ent.position[2],
                    );
                    gizmos.line(
                        pos + Vec3::Y * 0.45,
                        inf_pos,
                        Color::srgba(0.88, 0.35, 1.0, 0.90),
                    );
                }
            }
            // Draw Patrol Route loop line & end beacons when patrolling
            if let Some((p_start, p_end)) = sim_ent.patrol_route {
                let pt_a = Vec3::new(p_start[0], 0.18, p_start[2]);
                let pt_b = Vec3::new(p_end[0], 0.18, p_end[2]);
                let patrol_col = Color::srgba(0.25, 0.82, 1.0, 0.82);
                gizmos.line(pt_a, pt_b, patrol_col);
                gizmos.circle(
                    Isometry3d::new(pt_a, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    0.45,
                    patrol_col,
                );
                gizmos.circle(
                    Isometry3d::new(pt_b, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    0.45,
                    patrol_col,
                );
            }

            // Draw Guard escort link line when guarding a friendly entity
            if let Some(g_id) = sim_ent.guard_target_id {
                if let Some(g_ent) = world.entities.get(&g_id) {
                    let g_pos = Vec3::new(
                        g_ent.position[0],
                        g_ent.position[1].max(0.12) + 0.45,
                        g_ent.position[2],
                    );
                    gizmos.line(
                        pos + Vec3::Y * 0.45,
                        g_pos,
                        Color::srgba(0.22, 0.95, 0.88, 0.85),
                    );
                }
            }

            // Draw Force-Fire Ground (AttackGround) target crosshair & bombardment line
            if let Some(ag_pos) = sim_ent.attack_ground_pos {
                let ag_pt = Vec3::new(ag_pos[0], ag_pos[1].max(0.16), ag_pos[2]);
                let ag_col = Color::srgba(1.0, 0.25, 0.12, 0.88);
                gizmos.line(pos + Vec3::Y * 0.45, ag_pt, ag_col);
                gizmos.circle(
                    Isometry3d::new(ag_pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    1.25,
                    ag_col,
                );
                gizmos.line(
                    ag_pt + Vec3::new(-0.9, 0.04, 0.0),
                    ag_pt + Vec3::new(0.9, 0.04, 0.0),
                    ag_col,
                );
                gizmos.line(
                    ag_pt + Vec3::new(0.0, 0.04, -0.9),
                    ag_pt + Vec3::new(0.0, 0.04, 0.9),
                    ag_col,
                );
            }

            // Draw Mobile Medic / Mechanic Healing Aura & Cloak Detector Sensor Ring
            if let EntityKind::Unit { unit_id } = &sim_ent.kind {
                if let Some(u_def) = loaded.units.get(unit_id) {
                    if u_def.support_aura.mobile_heal_per_sec > 0.0
                        && u_def.support_aura.mobile_heal_radius > 0.1
                    {
                        gizmos.circle(
                            Isometry3d::new(
                                Vec3::new(pos.x, 0.12, pos.z),
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            u_def.support_aura.mobile_heal_radius,
                            Color::srgba(0.18, 0.98, 0.48, 0.42),
                        );
                    }
                    if u_def.stealth.detects_cloak {
                        gizmos.circle(
                            Isometry3d::new(
                                Vec3::new(pos.x, 0.14, pos.z),
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            u_def.vision_range,
                            Color::srgba(0.25, 0.88, 1.0, 0.28),
                        );
                    }
                }
            }
            if interaction.dev_mode_enabled {
                let radius = match &sim_ent.kind {
                    EntityKind::Unit { .. } => 1.35,
                    EntityKind::Building { .. } => 2.6,
                };
                gizmos.circle(
                    Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    radius,
                    Color::srgb(0.15, 0.95, 0.25),
                );
            } else {
                // Authentic C&C / Red Alert 2 white corner selection brackets `[ ]`
                let (hw, hd, leg) = match &sim_ent.kind {
                    EntityKind::Unit { .. } => (0.92, 0.92, 0.36),
                    EntityKind::Building { .. } => (2.15, 2.15, 0.68),
                };
                let by = pos.y + 0.08;
                let brk_col = Color::srgb(0.96, 0.98, 1.0);
                for sx in [-1.0_f32, 1.0_f32] {
                    for sz in [-1.0_f32, 1.0_f32] {
                        let corner = Vec3::new(pos.x + sx * hw, by, pos.z + sz * hd);
                        let end_x = Vec3::new(pos.x + sx * (hw - leg), by, pos.z + sz * hd);
                        let end_z = Vec3::new(pos.x + sx * hw, by, pos.z + sz * (hd - leg));
                        gizmos.line(corner, end_x, brk_col);
                        gizmos.line(corner, end_z, brk_col);
                    }
                }
            }

            // Draw 3D weapon firing range ring only in Engine Developer Mode [F12] or when aiming Attack-Move [X]
            if (interaction.dev_mode_enabled || interaction.attack_move_mode)
                && interaction.selected_ids.len() <= 4
            {
                if let Some(w_id) = &sim_ent.primary_weapon {
                    if let Some(w_def) = loaded.weapons.get(w_id) {
                        gizmos.circle(
                            Isometry3d::new(
                                Vec3::new(pos.x, 0.08, pos.z),
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            w_def.range,
                            Color::srgba(1.0, 0.42, 0.20, 0.28),
                        );
                    }
                }
            }

            // Draw full multi-segment waypoint chain (including Shift+RMB queued paths & Reverse Move)
            if !sim_ent.path_waypoints.is_empty() {
                let line_col = if sim_ent.reverse_moving {
                    Color::srgba(0.95, 0.78, 0.18, 0.85)
                } else if sim_ent.attack_move_dest.is_some()
                    || sim_ent.target_entity_id.is_some()
                {
                    Color::srgba(1.0, 0.30, 0.22, 0.72)
                } else {
                    Color::srgba(0.20, 0.95, 0.42, 0.65)
                };
                let mut prev = pos;
                for wp in &sim_ent.path_waypoints {
                    let next_pt = Vec3::new(wp[0], 0.16, wp[2]);
                    gizmos.line(prev, next_pt, line_col);
                    prev = next_pt;
                }
                gizmos.circle(
                    Isometry3d::new(prev, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    0.42,
                    line_col,
                );
            }

            // Draw factory Rally Point line, multi-waypoint chain (`rally_waypoints`), & 3D beacon if set on a selected building
            if !sim_ent.rally_waypoints.is_empty() || sim_ent.rally_point.is_some() {
                let gold = Color::srgba(0.98, 0.82, 0.18, 0.88);
                let mut prev = pos + Vec3::Y * 0.6;
                let chain: Vec<[f32; 3]> = if !sim_ent.rally_waypoints.is_empty() {
                    sim_ent.rally_waypoints.clone()
                } else {
                    sim_ent.rally_point.into_iter().collect()
                };
                for (idx, r_pos) in chain.iter().enumerate() {
                    let rally_pt = Vec3::new(r_pos[0], 0.16, r_pos[2]);
                    gizmos.line(prev, rally_pt, gold);
                    gizmos.circle(
                        Isometry3d::new(
                            rally_pt,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        if idx + 1 == chain.len() { 0.85 } else { 0.48 },
                        gold,
                    );
                    if idx + 1 == chain.len() {
                        // Vertical rally flag pole & pennant at final waypoint
                        gizmos.line(rally_pt, rally_pt + Vec3::Y * 2.2, gold);
                        gizmos.line(
                            rally_pt + Vec3::Y * 2.2,
                            rally_pt + Vec3::new(0.75, 1.85, 0.0),
                            gold,
                        );
                        gizmos.line(
                            rally_pt + Vec3::new(0.75, 1.85, 0.0),
                            rally_pt + Vec3::Y * 1.5,
                            gold,
                        );
                    }
                    prev = rally_pt;
                }
            }
        }

        // Draw Temporal Erase instability / Chrono-lock vortex ring (`temporal_progress > 0.0` or `is_phased_out()`)
        if sim_ent.temporal_progress > 0.0 || sim_ent.is_phased_out() {
            let chrono_col = Color::srgba(0.42, 0.88, 1.0, 0.90);
            let frac = (sim_ent.temporal_progress / sim_ent.max_health.max(1.0)).clamp(0.0, 1.0);
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * (0.45 + frac * 1.1),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.35 * (1.0 - frac * 0.45),
                chrono_col,
            );
        }

        // Draw Gatling Spin-Up rapid-fire ring (`spin_up_level > 0.05`)
        if sim_ent.spin_up_level > 0.05 {
            let spin_col = Color::srgba(1.0, 0.68, 0.15, 0.85);
            let spin_r = 1.15 + sim_ent.spin_up_level.min(2.0) * 0.35;
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.55,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                spin_r,
                spin_col,
            );
        }

        // Draw Laser Target Designator crosshairs & sky-beam (`pending_airstrike_timer > 0.0`)
        if sim_ent.pending_airstrike_timer > 0.0 {
            let laser_col = Color::srgba(1.0, 0.12, 0.18, 0.95);
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.35,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                2.4,
                laser_col,
            );
            gizmos.line(pos + Vec3::Y * 0.35, pos + Vec3::Y * 7.5, laser_col);
        }

        // Draw Tentacle Grapple constriction rings (`is_grappled()`)
        if sim_ent.is_grappled() {
            let grapple_col = Color::srgba(0.88, 0.18, 0.52, 0.90);
            for h in [0.35_f32, 0.75, 1.15] {
                gizmos.circle(
                    Isometry3d::new(
                        pos + Vec3::Y * h,
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                    ),
                    1.45,
                    grapple_col,
                );
            }
        }

        // Draw Offline Robot-Control link ring when an unmanned unit loses its control center (`is_entity_control_offline`)
        if world.is_entity_control_offline(sim_ent, loaded) {
            let offline_col = Color::srgba(0.95, 0.55, 0.12, 0.88);
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 1.45,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                0.78,
                offline_col,
            );
        }

        // Draw amber sonar/firing-reveal ring when a cloaked submarine or stealth unit uncloaks on fire (`reveal_timer > 0.0`)
        if sim_ent.reveal_timer > 0.0 {
            let rev_col = Color::srgba(1.0, 0.72, 0.16, 0.90);
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.32,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.55,
                rev_col,
            );
        }

        // Draw cyan holo-ring around factories actively producing a unit
        if !sim_ent.production_queue.is_empty() {
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.18,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                2.85,
                Color::srgba(0.25, 0.85, 1.0, 0.55),
            );
        }

        // Draw 3D Force-Shield / Iron-Curtain invulnerability energy dome (`is_invulnerable()`)
        if sim_ent.is_invulnerable() {
            let is_bldg = matches!(sim_ent.kind, EntityKind::Building { .. });
            let dome_r = if is_bldg { 3.1 } else { 1.45 };
            let shield_col = if is_bldg {
                Color::srgba(0.20, 0.78, 1.0, 0.78)
            } else {
                Color::srgba(0.95, 0.16, 0.18, 0.80)
            };
            gizmos.sphere(
                Isometry3d::from_translation(pos + Vec3::Y * (dome_r * 0.45)),
                dome_r,
                shield_col,
            );
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.22,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                dome_r * 1.05,
                shield_col,
            );
        }

        // Draw Cloning Vats bio-ring & Superweapon alert ring on completed buildings
        if let EntityKind::Building { building_id } = &sim_ent.kind {
            if sim_ent.construction_progress >= 1.0 {
                if let Some(b_def) = loaded.buildings.get(building_id) {
                    if b_def.cloning_vats {
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * 0.20,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            2.45,
                            Color::srgba(0.76, 0.22, 0.96, 0.72),
                        );
                    }
                    if b_def.container.power_per_occupant > 0 && !sim_ent.passenger_ids.is_empty() {
                        let bio_col = Color::srgba(0.32, 1.0, 0.45, 0.88);
                        for idx in 0..sim_ent.passenger_ids.len().min(5) {
                            gizmos.circle(
                                Isometry3d::new(
                                    pos + Vec3::Y * (0.35 + idx as f32 * 0.32),
                                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                                ),
                                1.95,
                                bio_col,
                            );
                        }
                    }
                    if b_def.container.passengers_can_fire && !sim_ent.passenger_ids.is_empty() {
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * 0.32,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            2.15,
                            Color::srgba(1.0, 0.72, 0.22, 0.88),
                        );
                    }
                    if b_def.superweapon.is_some() {
                        let sw_col = if sim_ent.ability_cooldown <= 0.0 {
                            Color::srgba(1.0, 0.18, 0.15, 0.88)
                        } else {
                            Color::srgba(1.0, 0.65, 0.15, 0.68)
                        };
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * 0.25,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            3.35,
                            sw_col,
                        );
                    }
                    if sim_ent.power_disabled {
                        let off_col = Color::srgba(1.0, 0.42, 0.08, 0.88);
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * 0.28,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            2.1,
                            off_col,
                        );
                        gizmos.line(
                            pos + Vec3::new(-1.1, 3.2, -1.1),
                            pos + Vec3::new(1.1, 3.2, 1.1),
                            off_col,
                        );
                    }
                    if sim_ent.has_power_charge_boost() {
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * 0.30,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            1.85,
                            Color::srgba(0.32, 0.92, 1.0, 0.88),
                        );
                    }
                    if b_def.stealth.psychic_sensor_radius > 0.0 && is_selected {
                        gizmos.circle(
                            Isometry3d::new(
                                pos + Vec3::Y * 0.14,
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            b_def.stealth.psychic_sensor_radius,
                            Color::srgba(0.85, 0.22, 0.96, 0.45),
                        );
                    }
                }
            }
        }

        // Draw golden PRIMARY factory crown indicator on Primary Buildings when selected or in Developer Mode
        if sim_ent.is_primary
            && sim_ent.player_id == local_pid
            && (is_selected || interaction.dev_mode_enabled)
        {
            let crown_y = base_y + 4.15;
            let gold = Color::srgb(1.0, 0.88, 0.18);
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(pos.x, crown_y, pos.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                0.55,
                gold,
            );
        }

        // Draw 3D Aircraft Ammo Pips (`current_ammo / max_ammo`) above aircraft
        if sim_ent.max_ammo > 0 {
            let ammo_y = base_y + 1.55;
            let spacing = 0.36_f32;
            let start_x = pos.x - (sim_ent.max_ammo as f32 - 1.0) * spacing * 0.5;
            for slot in 0..sim_ent.max_ammo {
                let px = start_x + slot as f32 * spacing;
                let loaded_round = slot < sim_ent.current_ammo;
                let col = if loaded_round {
                    Color::srgb(1.0, 0.72, 0.15)
                } else {
                    Color::srgba(0.35, 0.35, 0.40, 0.65)
                };
                gizmos.cuboid(
                    Transform::from_xyz(px, ammo_y, pos.z).with_scale(Vec3::new(0.22, 0.14, 0.14)),
                    col,
                );
            }
        }

        if sim_ent.script_ctx.deployed {
            let dep_col = Color::srgb(0.98, 0.72, 0.12);
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.15,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.65,
                dep_col,
            );
            if let EntityKind::Unit { unit_id } = &sim_ent.kind {
                if loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.lands_when_deployed)
                    .unwrap_or(false)
                {
                    gizmos.line(
                        pos + Vec3::new(-1.85, 0.18, 0.0),
                        pos + Vec3::new(1.85, 0.18, 0.0),
                        dep_col,
                    );
                    gizmos.line(
                        pos + Vec3::new(0.0, 0.18, -1.85),
                        pos + Vec3::new(0.0, 0.18, 1.85),
                        dep_col,
                    );
                }
            }
        }
        if sim_ent.has_c4_bomb() {
            let bomb_col = Color::srgba(1.0, 0.22, 0.08, 0.95);
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * 0.42,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.55,
                bomb_col,
            );
            gizmos.line(pos + Vec3::Y * 0.42, pos + Vec3::Y * 2.8, bomb_col);
        }
        if sim_ent.is_confused() {
            gizmos.circle(
                Isometry3d::new(
                    pos + Vec3::Y * (base_y + 2.85),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                0.85,
                Color::srgba(0.92, 0.18, 0.96, 0.88),
            );
        }

        // Draw golden Veteran / Elite chevrons above promoted units
        if sim_ent.veterancy != VeterancyRank::Rookie {
            let chev_y = base_y + 2.45;
            let gold = Color::srgb(1.0, 0.85, 0.15);
            let a = Vec3::new(pos.x - 0.35, chev_y, pos.z);
            let top = Vec3::new(pos.x, chev_y + 0.25, pos.z);
            let b = Vec3::new(pos.x + 0.35, chev_y, pos.z);
            gizmos.line(a, top, gold);
            gizmos.line(top, b, gold);

            if sim_ent.veterancy == VeterancyRank::Elite {
                let a2 = Vec3::new(pos.x - 0.35, chev_y + 0.22, pos.z);
                let top2 = Vec3::new(pos.x, chev_y + 0.47, pos.z);
                let b2 = Vec3::new(pos.x + 0.35, chev_y + 0.22, pos.z);
                gizmos.line(a2, top2, gold);
                gizmos.line(top2, b2, gold);
            }
        }

        if is_selected || sim_ent.health < sim_ent.max_health {
            let ratio = (sim_ent.health / sim_ent.max_health.max(1.0)).clamp(0.0, 1.0);
            let bar_y = match &sim_ent.kind {
                EntityKind::Unit { .. } => base_y + 1.95,
                EntityKind::Building { .. } => 3.6,
            };
            let half_w = match &sim_ent.kind {
                EntityKind::Unit { .. } => 0.78,
                EntityKind::Building { .. } => 1.45,
            };
            let left = Vec3::new(pos.x - half_w, bar_y, pos.z);
            let right_full = Vec3::new(pos.x + half_w, bar_y, pos.z);
            let right_hp = Vec3::new(pos.x - half_w + (2.0 * half_w) * ratio, bar_y, pos.z);
            let hp_color = if ratio > 0.5 {
                Color::srgb(0.18, 0.98, 0.30)
            } else if ratio > 0.25 {
                Color::srgb(0.98, 0.82, 0.16)
            } else {
                Color::srgb(0.98, 0.22, 0.15)
            };
            // White RA2 health bar frame ticks at ends + double-line thickness
            let frame_col = Color::srgb(0.95, 0.97, 1.0);
            gizmos.line(
                Vec3::new(pos.x - half_w, bar_y - 0.06, pos.z),
                Vec3::new(pos.x - half_w, bar_y + 0.06, pos.z),
                frame_col,
            );
            gizmos.line(
                Vec3::new(pos.x + half_w, bar_y - 0.06, pos.z),
                Vec3::new(pos.x + half_w, bar_y + 0.06, pos.z),
                frame_col,
            );
            if ratio > 0.001 {
                gizmos.line(left, right_hp, hp_color);
                gizmos.line(
                    left + Vec3::Y * 0.035,
                    right_hp + Vec3::Y * 0.035,
                    hp_color,
                );
            }
            if ratio < 0.999 {
                gizmos.line(right_hp, right_full, Color::srgb(0.22, 0.06, 0.06));
            }
        }
    }

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_world_and_placement_gizmos(
    gizmos: &mut Gizmos,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    local_pid: u8,
    elapsed: f32,
) {

    // Draw 3D Landmines (`SimWorld.mines`) when friendly/allied or revealed by a detector
    for mine in &world.mines {
        let is_friendly =
            mine.owner_player_id == local_pid || !world.are_hostile(local_pid, mine.owner_player_id);
        if is_friendly || mine.revealed_to_enemies {
            let m_pos = Vec3::new(mine.position[0], mine.position[1] + 0.08, mine.position[2]);
            let mine_col = if is_friendly {
                Color::srgba(0.25, 0.92, 0.45, 0.85)
            } else {
                Color::srgba(1.0, 0.25, 0.18, 0.92)
            };
            gizmos.circle(
                Isometry3d::new(m_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                0.55,
                mine_col,
            );
            gizmos.cuboid(
                Transform::from_translation(m_pos + Vec3::Y * 0.05)
                    .with_scale(Vec3::new(0.45, 0.12, 0.45)),
                mine_col,
            );
        }
    }

    // Draw 3D Destructible & Explosive Map Props (`SimWorld.props`: Trees, Streetlights, ExplosiveBarrels)
    for prop in &world.props {
        let p_pos = Vec3::new(prop.position[0], prop.position[1], prop.position[2]);
        match prop.kind {
            PropKind::Tree => {
                let trunk_top = p_pos + Vec3::Y * 1.45;
                gizmos.line(p_pos, trunk_top, Color::srgb(0.42, 0.26, 0.14));
                gizmos.sphere(
                    Isometry3d::from_translation(trunk_top + Vec3::Y * 0.45),
                    0.85,
                    Color::srgba(0.16, 0.58, 0.22, 0.85),
                );
            }
            PropKind::Streetlight => {
                let lamp_top = p_pos + Vec3::Y * 2.35;
                gizmos.line(p_pos, lamp_top, Color::srgb(0.55, 0.60, 0.66));
                let glow_col = if matches!(
                    interaction.environment_preset,
                    EnvironmentPreset::Night | EnvironmentPreset::Dusk
                ) {
                    Color::srgba(1.0, 0.92, 0.45, 0.95)
                } else {
                    Color::srgba(0.85, 0.88, 0.72, 0.55)
                };
                gizmos.sphere(Isometry3d::from_translation(lamp_top), 0.24, glow_col);
            }
            PropKind::ExplosiveBarrel => {
                let b_center = p_pos + Vec3::Y * 0.42;
                gizmos.cuboid(
                    Transform::from_translation(b_center).with_scale(Vec3::new(0.52, 0.82, 0.52)),
                    Color::srgb(0.95, 0.22, 0.12),
                );
                gizmos.circle(
                    Isometry3d::new(
                        p_pos + Vec3::Y * 0.06,
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                    ),
                    0.62,
                    Color::srgba(1.0, 0.55, 0.12, 0.72),
                );
            }
        }
    }

    // Draw Collectible 3D Bonus Crates (`SimWorld.crates`)
    for crate_item in &world.crates {
        let c_pos = Vec3::new(
            crate_item.position[0],
            crate_item.position[1] + 0.55 + (elapsed * 2.5).sin() * 0.12,
            crate_item.position[2],
        );
        let crate_col = match crate_item.kind {
            CrateKind::BonusCredits => Color::srgb(1.0, 0.85, 0.18),
            CrateKind::HealAll => Color::srgb(0.22, 0.98, 0.45),
            CrateKind::VeterancyPromo => Color::srgb(1.0, 0.58, 0.15),
            CrateKind::FirepowerBoost => Color::srgb(1.0, 0.25, 0.25),
            CrateKind::SpeedBoost => Color::srgb(0.25, 0.88, 1.0),
            CrateKind::RevealMap => Color::srgb(0.75, 0.45, 1.0),
            CrateKind::SalvageScrap => Color::srgb(0.85, 0.92, 1.0),
            CrateKind::CloakStealth => Color::srgb(0.35, 0.96, 0.90),
            CrateKind::FreeUnit => Color::srgb(0.45, 1.0, 0.25),
        };
        gizmos.cuboid(
            Transform::from_translation(c_pos)
                .with_rotation(Quat::from_rotation_y(elapsed * 1.6 + crate_item.id as f32))
                .with_scale(Vec3::splat(0.78)),
            crate_col,
        );
        gizmos.circle(
            Isometry3d::new(
                Vec3::new(c_pos.x, crate_item.position[1] + 0.10, c_pos.z),
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.95,
            crate_col,
        );
    }

    // Draw 3D Tactical Pings (`SimWorld.active_pings`)
    for ping in &world.active_pings {
        let p_pos = Vec3::new(ping.position[0], 0.18, ping.position[2]);
        let pulse = 1.0 + ((4.5 - ping.remaining_secs) * 3.5) % 3.2;
        let ping_col = Color::srgba(1.0, 0.88, 0.20, (ping.remaining_secs / 4.5).clamp(0.2, 0.95));
        gizmos.circle(
            Isometry3d::new(p_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            pulse,
            ping_col,
        );
        gizmos.line(p_pos, p_pos + Vec3::Y * 5.5, ping_col);
    }

    // Draw King of the Hill Center Capture Zone (`GameMode::KingOfTheHill`)
    if world.game_mode == GameMode::KingOfTheHill {
        let center_w = world
            .grid
            .tile_to_world([world.grid.width / 2, world.grid.height / 2]);
        let koth_pos = Vec3::new(center_w[0], 0.16, center_w[2]);
        gizmos.circle(
            Isometry3d::new(koth_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            10.0,
            Color::srgba(1.0, 0.82, 0.18, 0.65),
        );
    }

    // Draw active Radiation & Biotoxin Zones (Desolator / RadBeam / Nuke fallout / Virus Biotoxin cloud)
    for zone in &world.radiation_zones {
        let z_pos = Vec3::new(zone.position[0], 0.15, zone.position[2]);
        let alpha = (zone.remaining_secs / 6.0).clamp(0.2, 0.85);
        let (outer_col, inner_col) = if zone.is_toxic_cloud {
            (
                Color::srgba(0.65, 0.98, 0.22, alpha),
                Color::srgba(0.78, 0.28, 0.96, alpha * 0.85),
            )
        } else {
            (
                Color::srgba(0.22, 1.0, 0.18, alpha),
                Color::srgba(0.45, 1.0, 0.25, alpha * 0.8),
            )
        };
        gizmos.circle(
            Isometry3d::new(z_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            zone.radius,
            outer_col,
        );
        gizmos.circle(
            Isometry3d::new(
                z_pos + Vec3::Y * 0.05,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            zone.radius * 0.55,
            inner_col,
        );
    }

    // Draw Persistent 3D Battlefield Scars: Explosion Craters & Smoking Vehicle Wrecks (`SimWorld.battlefield_scars`)
    for scar in &world.battlefield_scars {
        let s_pos = Vec3::new(scar.position[0], scar.position[1].max(0.04), scar.position[2]);
        let fade = (scar.remaining_secs / 45.0).clamp(0.18, 0.85);
        let scorch_col = Color::srgba(0.18, 0.16, 0.14, fade);
        gizmos.circle(
            Isometry3d::new(s_pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            scar.radius,
            scorch_col,
        );
        if scar.is_wreck {
            let hulk_col = Color::srgba(0.28, 0.25, 0.22, fade);
            gizmos.cuboid(
                Transform::from_translation(s_pos + Vec3::Y * 0.28)
                    .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.18, scar.position[0], -0.14))
                    .with_scale(Vec3::new(1.25, 0.55, 1.65)),
                hulk_col,
            );
        }
    }

    // Draw building placement preview ghost + local 3D tactical tile grid + Support Power targeting reticle on the terrain
    if !interaction.studio_open {
        // Draw 3D Squad Formation Target Preview rings at cursor when multiple units are selected
        if interaction.placing_building_id.is_none()
            && interaction.active_support_power_id.is_none()
            && interaction.selected_ids.len() >= 2
            && interaction.selected_ids.len() <= 24
        {
            let has_movable_unit = interaction.selected_ids.iter().any(|id| {
                world
                    .entities
                    .get(id)
                    .map(|e| e.player_id == local_pid && matches!(e.kind, EntityKind::Unit { .. }))
                    .unwrap_or(false)
            });
            if has_movable_unit {
                let hov_world = world.grid.tile_to_world(interaction.hovered_tile);
                let sel_vec: Vec<u64> = interaction.selected_ids.iter().copied().collect();
                let form_targets = world.compute_squad_formation_targets(
                    &sel_vec,
                    hov_world,
                    loaded,
                );
                for &t_tile in form_targets.values() {
                    let tw = world.grid.tile_to_world(t_tile);
                    gizmos.circle(
                        Isometry3d::new(
                            Vec3::new(tw[0], tw[1].max(0.10), tw[2]),
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        0.46,
                        Color::srgba(0.25, 0.96, 0.55, 0.42),
                    );
                }
            }
        }

        if let Some(power_id) = &interaction.active_support_power_id {
            let hov_pos = world.grid.tile_to_world(interaction.hovered_tile);
            let target_pt = Vec3::new(hov_pos[0], hov_pos[1].max(0.14), hov_pos[2]);
            let sp_rad = loaded
                .buildings
                .values()
                .flat_map(|b| &b.support_powers)
                .find(|sp| &sp.id == power_id)
                .map(|sp| sp.radius.max(3.5))
                .unwrap_or(6.0);
            let reticle_col = Color::srgba(1.0, 0.68, 0.16, 0.88);
            gizmos.circle(
                Isometry3d::new(
                    target_pt,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                sp_rad,
                reticle_col,
            );
            gizmos.circle(
                Isometry3d::new(
                    target_pt + Vec3::Y * 0.04,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                1.1,
                reticle_col,
            );
            if let Some(src) = interaction.chrono_shift_source_pos {
                let src_pt = Vec3::new(src[0], 0.20, src[2]);
                let chrono_col = Color::srgba(0.25, 0.82, 1.0, 0.90);
                gizmos.circle(
                    Isometry3d::new(src_pt, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    sp_rad,
                    chrono_col,
                );
                gizmos.line(src_pt, target_pt + Vec3::Y * 0.20, chrono_col);
            }
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

                // Draw local 7x7 tile grid cells around cursor
                for dz in -3i32..=4i32 {
                    for dx in -3i32..=4i32 {
                        let t = [hov[0] + dx, hov[1] + dz];
                        if !world.grid.in_bounds(t) {
                            continue;
                        }
                        let walkable = world.grid.is_walkable(t);
                        let cell_pos = world.grid.tile_to_world(t);
                        let cell_col = if walkable {
                            Color::srgba(0.20, 0.92, 0.40, 0.22)
                        } else {
                            Color::srgba(0.95, 0.22, 0.22, 0.35)
                        };
                        gizmos.cuboid(
                            Transform::from_xyz(cell_pos[0], 0.04, cell_pos[2])
                                .with_scale(Vec3::new(ts * 0.92, 0.02, ts * 0.92)),
                            cell_col,
                        );
                    }
                }

                let within_radius = world.is_within_build_radius(
                    local_pid,
                    interaction.hovered_tile,
                    active_fp,
                );
                let footprint_ok = if b_def.role == BuildingRole::Bridge || b_def.water_bound {
                    (0..active_fp[1] as i32).all(|dz| {
                        (0..active_fp[0] as i32).all(|dx| {
                            world.grid.in_bounds([
                                interaction.hovered_tile[0] + dx,
                                interaction.hovered_tile[1] + dz,
                            ])
                        })
                    })
                } else {
                    world
                        .grid
                        .can_place_footprint(interaction.hovered_tile, active_fp)
                };
                let can_place = footprint_ok && within_radius;

                // Draw per-tile footprint sub-cells with individual green/red validity highlight
                for dz in 0..active_fp[1] as i32 {
                    for dx in 0..active_fp[0] as i32 {
                        let sub_t = [hov[0] + dx, hov[1] + dz];
                        let sub_ok = world.grid.in_bounds(sub_t)
                            && (b_def.role == BuildingRole::Bridge
                                || b_def.water_bound
                                || world.grid.is_walkable(sub_t))
                            && within_radius;
                        let sub_pos = world.grid.tile_to_world(sub_t);
                        let sub_col = if sub_ok {
                            Color::srgba(0.15, 1.0, 0.38, 0.55)
                        } else {
                            Color::srgba(1.0, 0.18, 0.18, 0.65)
                        };
                        gizmos.cuboid(
                            Transform::from_xyz(sub_pos[0], sub_pos[1] + 0.08, sub_pos[2])
                                .with_scale(Vec3::new(ts * 0.96, 0.06, ts * 0.96)),
                            sub_col,
                        );
                    }
                }

                let center = world
                    .grid
                    .footprint_center_world(interaction.hovered_tile, active_fp);
                let w = active_fp[0] as f32 * ts;
                let d = active_fp[1] as f32 * ts;
                let color = if can_place {
                    Color::srgb(0.2, 0.95, 0.35)
                } else {
                    Color::srgb(0.95, 0.2, 0.2)
                };
                if let Some(start_tile) = interaction.wall_drag_start_tile {
                    let line_tiles =
                        rts_simulation::pathfinding::CollisionGrid::compute_line_tiles(
                            start_tile,
                            interaction.hovered_tile,
                        );
                    for lt in line_tiles {
                        let lt_center = world.grid.footprint_center_world(lt, active_fp);
                        let lt_ok = world.grid.can_place_footprint(lt, active_fp)
                            && world.is_within_build_radius(local_pid, lt, active_fp);
                        let lt_col = if lt_ok {
                            Color::srgb(0.2, 0.95, 0.35)
                        } else {
                            Color::srgb(0.95, 0.2, 0.2)
                        };
                        gizmos.cuboid(
                            Transform::from_xyz(lt_center[0], lt_center[1] + 0.95, lt_center[2])
                                .with_scale(Vec3::new(w * 0.94, 1.9, d * 0.94)),
                            lt_col,
                        );
                    }
                } else {
                    gizmos.cuboid(
                        Transform::from_xyz(center[0], center[1] + 1.1, center[2])
                            .with_scale(Vec3::new(w, 2.2, d)),
                        color,
                    );
                }

                // Draw C&C Construction Radius ring around friendly base structures during placement
                for ent in world.entities.values() {
                    if ent.player_id == local_pid
                        && ent.health > 0.0
                        && matches!(ent.kind, EntityKind::Building { .. })
                    {
                        gizmos.circle(
                            Isometry3d::new(
                                Vec3::new(ent.position[0], 0.10, ent.position[2]),
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            24.0 * ts,
                            Color::srgba(0.22, 0.78, 1.0, 0.16),
                        );
                    }
                }

                // If placing a defensive structure with a weapon, preview its firing range
                if let Some(w_id) = &b_def.primary_weapon {
                    if let Some(w_def) = loaded.weapons.get(w_id) {
                        gizmos.circle(
                            Isometry3d::new(
                                Vec3::new(center[0], 0.12, center[2]),
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                            ),
                            w_def.range,
                            Color::srgba(1.0, 0.55, 0.18, 0.48),
                        );
                    }
                }
            }
        }
    }

    // Draw in-flight 3D Ballistic Shells & Homing Missiles (`SimWorld.projectiles`)
    for proj in &world.projectiles {
        let p_pos = Vec3::from_array(proj.current_pos);
        let s_pos = Vec3::from_array(proj.start_pos);
        let t_pos = Vec3::from_array(proj.target_pos);
        let trail_start = p_pos.lerp(s_pos, 0.18);
        let proj_col = match proj.delivery {
            rts_sdk::schema::DeliveryKind::Missile => Color::srgb(1.0, 0.42, 0.15),
            _ => Color::srgb(1.0, 0.85, 0.25),
        };
        gizmos.line(trail_start, p_pos, proj_col);
        gizmos.sphere(Isometry3d::from_translation(p_pos), 0.28, proj_col);
        if proj.max_health > 0.0 {
            let ratio = (proj.health / proj.max_health).clamp(0.0, 1.0);
            let bar_y = p_pos.y + 0.55;
            let left = Vec3::new(p_pos.x - 0.55, bar_y, p_pos.z);
            let right_full = Vec3::new(p_pos.x + 0.55, bar_y, p_pos.z);
            let right_hp = Vec3::new(p_pos.x - 0.55 + 1.10 * ratio, bar_y, p_pos.z);
            gizmos.line(left, right_full, Color::srgb(0.25, 0.05, 0.05));
            gizmos.line(left, right_hp, Color::srgb(1.0, 0.62, 0.15));
        }
        if proj.splash_radius > 0.5 {
            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(t_pos.x, t_pos.y + 0.08, t_pos.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                proj.splash_radius * 0.45,
                Color::srgba(1.0, 0.35, 0.15, 0.38),
            );
        }
    }

    // Draw Yuri Psychic Sensor threat warning lines (`psychic_sensor_threat_lines`) for local player
    for (att_pos, tgt_pos) in world.psychic_sensor_threat_lines(local_pid, loaded) {
        let a = Vec3::new(att_pos[0], att_pos[1].max(0.35), att_pos[2]);
        let b = Vec3::new(tgt_pos[0], tgt_pos[1].max(0.35), tgt_pos[2]);
        let psi_warn_col = Color::srgba(0.92, 0.18, 0.96, 0.85);
        gizmos.line(a, b, psi_warn_col);
        gizmos.circle(
            Isometry3d::new(
                Vec3::new(b.x, 0.22, b.z),
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            1.65,
            psi_warn_col,
        );
    }
}
