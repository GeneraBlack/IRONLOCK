use super::minimap_render::{
    composite_radar_frame, compute_radar_dynamic_fingerprint, fnv_mix,
    rebuild_static_radar_terrain_256, CachedRadarBaseTerrain, RADAR_TEX_SIZE,
};
use super::*;
use crate::audio::{SoundEffectKind, SoundSettingsResource, UnitVoiceCategory};
use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState};
use crate::rendering::{CachedFogVisibility, PreviewStageCamera, VideoPlaybackResource};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingDef, BuildingRole, MapDef, SupportPowerKind};
use rts_simulation::command::{GameCommand, QueuedOrder};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::{EntityKind, SimWorld};

pub fn compute_terrain_static_digest(map: &MapDef, grid: &CollisionGrid) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325_u64;
    for b in map.id.as_bytes() {
        h = fnv_mix(h, u64::from(*b));
    }
    h = fnv_mix(h, u64::from(map.width));
    h = fnv_mix(h, u64::from(map.height));
    h = fnv_mix(h, grid.elevation_tiles.len() as u64);
    h = fnv_mix(h, grid.water_tiles.len() as u64);
    h = fnv_mix(h, grid.ramp_tiles.len() as u64);
    h = fnv_mix(h, grid.blocked_tiles.len() as u64);
    h = fnv_mix(h, map.obstacles.len() as u64);
    let splat_len = map
        .splatmap
        .as_ref()
        .map(|s| s.weights_rgba.len())
        .unwrap_or(0);
    h = fnv_mix(h, splat_len as u64);
    for (idx, &vh) in grid.vertex_heights.iter().step_by(7).enumerate() {
        h = fnv_mix(h, ((vh * 20.0).round() as i64 as u64) ^ (idx as u64));
    }
    if let Some(ref splat) = map.splatmap {
        for (idx, &[r, g, b, a]) in splat.weights_rgba.iter().step_by(19).enumerate() {
            let packed = u32::from_le_bytes([r, g, b, a]) as u64;
            h = fnv_mix(h, packed ^ ((idx as u64) << 8));
        }
    }
    h
}

pub fn fit_radar_box_dimensions(map_w: u32, map_h: u32, max_w: f32, max_h: f32) -> (f32, f32) {
    let mw = (map_w.max(1)) as f32;
    let mh = (map_h.max(1)) as f32;
    let map_aspect = mw / mh;
    let box_aspect = max_w / max_h.max(1.0);
    if map_aspect >= box_aspect {
        (max_w, (max_w / map_aspect).clamp(48.0, max_h))
    } else {
        ((max_h * map_aspect).clamp(48.0, max_w), max_h)
    }
}

/// Returns `true` when `b` acts as a tactical radar/communications provider structure.
pub fn is_radar_provider_building(b: &BuildingDef) -> bool {
    b.role == BuildingRole::Airfield
        || b.support_aura.grants_radar
        || b.stealth.psychic_sensor_radius > 0.0
        || b.support_powers
            .iter()
            .any(|sp| sp.kind == SupportPowerKind::RadarScan)
        || b.id.to_ascii_lowercase().contains("radar")
        || b.name.to_ascii_lowercase().contains("radar")
}

/// Checks whether `pid` currently has an operational, powered radar structure (when the faction defines one).
pub fn player_has_active_radar(world: &SimWorld, loaded: &LoadedMod, pid: u8) -> bool {
    let Some(econ) = world.players.get(&pid) else {
        return true;
    };
    let faction_defines_radar = loaded
        .buildings
        .values()
        .any(|b| b.is_available_to_faction(&econ.faction) && is_radar_provider_building(b));
    if !faction_defines_radar {
        return true;
    }
    if econ.is_low_power() || econ.power_sabotage_timer > 0.0 {
        return false;
    }
    world.entities.values().any(|e| {
        e.player_id == pid
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && !e.power_disabled
            && e.emp_remaining() <= 0.0
            && !e.is_phased_out()
            && matches!(&e.kind, EntityKind::Building { building_id }
                if loaded.buildings.get(building_id).is_some_and(is_radar_provider_building))
    })
}

/// Event-driven upload key (strictly NO `tick` field so idle frames perform zero CPU/GPU uploads!).
#[derive(Default, PartialEq, Eq)]
pub struct RadarUploadKey {
    pub initialized: bool,
    pub terrain_digest: u64,
    pub dynamic_fingerprint: u64,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_and_interact_radar_minimap(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    radar_res: Option<Res<RadarMinimapImage>>,
    mut video_res: Option<ResMut<VideoPlaybackResource>>,
    mut subwindows: Option<ResMut<PlayerSubWindowStack>>,
    cached_fow: Res<CachedFogVisibility>,
    mut images: ResMut<Assets<Image>>,
    (mod_res, mut sim_res, mut interaction, mut sound_res): (
        Res<ModResource>,
        ResMut<SimWorldResource>,
        ResMut<ClientInteractionState>,
        Option<ResMut<SoundSettingsResource>>,
    ),
    mut radar_node_q: Query<
        (&RelativeCursorPosition, &mut Node),
        (
            With<RadarMinimapNode>,
            Without<RadarPipVideoNode>,
            Without<HudLayoutRole>,
            Without<StudioModalRoot>,
            Without<SelectionDragBoxNode>,
            Without<super::player_windows::BriefingVideoProgressFill>,
        ),
    >,
    mut pip_video_q: Query<
        (&mut Node, &mut ImageNode),
        (
            With<RadarPipVideoNode>,
            Without<RadarMinimapNode>,
            Without<HudLayoutRole>,
            Without<StudioModalRoot>,
            Without<SelectionDragBoxNode>,
            Without<super::player_windows::BriefingVideoProgressFill>,
        ),
    >,
    mut camera_q: Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig),
        Without<PreviewStageCamera>,
    >,
    mut last_upload: Local<RadarUploadKey>,
    mut base_cache: Local<CachedRadarBaseTerrain>,
) {
    // 0. Sync In-Engine Radar PIP Video overlay whenever an incoming transmission plays outside Briefing modal
    let briefing_open = subwindows
        .as_deref()
        .is_some_and(|s| s.is_open(PlayerSubWindowKind::Briefing));
    let pip_frame = video_res
        .as_deref()
        .filter(|v| v.is_playing && !briefing_open)
        .map(|v| v.frame_image.clone());
    let pip_active = pip_frame.is_some();

    for (mut node, mut img_node) in &mut pip_video_q {
        if let Some(ref handle) = pip_frame {
            node.display = Display::Flex;
            if img_node.image != *handle {
                img_node.image = handle.clone();
            }
        } else {
            node.display = Display::None;
        }
    }

    if interaction.app_phase != AppPhase::InGame {
        return;
    }
    let Some(radar_img_res) = radar_res else {
        return;
    };
    let map = &mod_res.0.entry_map;
    let map_w = map.width.max(1) as f32;
    let map_h = map.height.max(1) as f32;
    let world_w = map_w * map.tile_size;
    let world_h = map_h * map.tile_size;

    // Aspect-ratio-preserving fit inside the 192x138 radar bezel so 1:1 maps never stretch!
    let (fit_w, fit_h) = fit_radar_box_dimensions(map.width, map.height, 192.0, 138.0);
    if let Ok((_, mut r_node)) = radar_node_q.get_single_mut() {
        let target_w = Val::Px(fit_w);
        let target_h = Val::Px(fit_h);
        if r_node.width != target_w {
            r_node.width = target_w;
        }
        if r_node.height != target_h {
            r_node.height = target_h;
        }
    }

    let pov_pid =
        if interaction.observer_pov_player_id > 0 && interaction.observer_pov_player_id != 255 {
            interaction.observer_pov_player_id
        } else {
            interaction.local_player_id
        };
    let enforce_radar_gate = !interaction.dev_mode_enabled
        && !interaction.replay_active
        && !interaction.spectator_mode_active
        && interaction.observer_pov_player_id == 0;
    let radar_online =
        !enforce_radar_gate || player_has_active_radar(&sim_res.0, &mod_res.0, pov_pid);

    // 1. Handle interactive mouse clicks inside the fitted Radar / PIP frame
    if !interaction.menu_open && !interaction.studio_open {
        if let Ok((rel_cursor, _)) = radar_node_q.get_single() {
            if let Some(norm) = rel_cursor.normalized {
                if (0.0..=1.0).contains(&norm.x) && (0.0..=1.0).contains(&norm.y) {
                    if pip_active {
                        if mouse_buttons.just_pressed(MouseButton::Left) {
                            if let Some(ref mut sw) = subwindows {
                                sw.open_or_focus(PlayerSubWindowKind::Briefing);
                            }
                        } else if mouse_buttons.just_pressed(MouseButton::Right) {
                            if let Some(ref mut vid) = video_res {
                                vid.stop();
                            }
                        }
                    } else if !radar_online {
                        if mouse_buttons.just_pressed(MouseButton::Left)
                            || mouse_buttons.just_pressed(MouseButton::Right)
                        {
                            interaction.push_eva_alert(
                                "EVA: Radar offline - build a Radar facility and maintain power.",
                            );
                        }
                    } else {
                        let wx = norm.x * world_w;
                        let wz = norm.y * world_h;
                        let wy = sim_res.0.grid.sample_world_height(wx, wz);
                        handle_radar_click(
                            &mouse_buttons,
                            &keys,
                            Vec3::new(wx, wy, wz),
                            &mut sim_res.0,
                            &mut interaction,
                            sound_res.as_deref_mut(),
                            &mut camera_q,
                        );
                    }
                }
            }
        }
    }

    let cam_quant = camera_q
        .get_single()
        .map(|(_, _, rig)| {
            [
                (rig.focus.x * 2.0).round() as i32,
                (rig.focus.z * 2.0).round() as i32,
                (rig.distance * 2.0).round() as i32,
                (rig.yaw_radians * 40.0).round() as i32,
                (rig.pitch_radians * 40.0).round() as i32,
            ]
        })
        .unwrap_or([0; 5]);

    let terrain_digest = compute_terrain_static_digest(map, &sim_res.0.grid);
    let fog_on = interaction.fog_of_war_enabled && !interaction.spectator_all_vision;
    let dynamic_fp = compute_radar_dynamic_fingerprint(
        &sim_res.0,
        &mod_res.0,
        &cached_fow,
        &interaction,
        pov_pid,
        fog_on,
        radar_online,
        cam_quant,
    );

    let key = RadarUploadKey {
        initialized: true,
        terrain_digest,
        dynamic_fingerprint: dynamic_fp,
    };
    if *last_upload == key {
        return;
    }
    *last_upload = key;

    if base_cache.dims != [map.width, map.height]
        || base_cache.terrain_digest != terrain_digest
        || base_cache.rgba_256.len() != RADAR_TEX_SIZE * RADAR_TEX_SIZE * 4
    {
        base_cache.map_id = map.id.clone();
        base_cache.dims = [map.width, map.height];
        base_cache.terrain_digest = terrain_digest;
        base_cache.rgba_256 = rebuild_static_radar_terrain_256(map, &sim_res.0.grid);
    }

    let Some(img) = images.get_mut(&radar_img_res.0) else {
        return;
    };
    let w_px = img.texture_descriptor.size.width as i32;
    let h_px = img.texture_descriptor.size.height as i32;
    if img.data.len() < (w_px * h_px * 4) as usize {
        return;
    }

    // 100% opaque (`alpha = 255`) offline radar standby screen so `radar.png` never bleeds through
    if !radar_online {
        for py in 0..h_px {
            for px in 0..w_px {
                let idx = ((py * w_px + px) * 4) as usize;
                let border = px <= 1 || py <= 1 || px >= w_px - 2 || py >= h_px - 2;
                let col = if border {
                    [96, 36, 36, 255]
                } else if py % 4 == 0 {
                    [12, 17, 22, 255]
                } else if px % 32 == 0 || py % 32 == 0 {
                    [18, 26, 34, 255]
                } else {
                    [8, 11, 16, 255]
                };
                img.data[idx..idx + 4].copy_from_slice(&col);
            }
        }
        return;
    }

    let cam_pair = camera_q.get_single().ok();
    composite_radar_frame(
        &mut img.data,
        &base_cache.rgba_256,
        w_px,
        h_px,
        &sim_res.0,
        &mod_res.0,
        &cached_fow,
        &interaction,
        pov_pid,
        fog_on,
        cam_pair.map(|(c, gt, _)| (c, gt)),
        cam_pair.map(|(_, _, rig)| rig),
    );
}

fn handle_radar_click(
    mouse_buttons: &ButtonInput<MouseButton>,
    keys: &ButtonInput<KeyCode>,
    target_world: Vec3,
    sim_world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
    sound_res: Option<&mut SoundSettingsResource>,
    camera_q: &mut Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig),
        Without<PreviewStageCamera>,
    >,
) {
    let pid = interaction.local_player_id;
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let pos = [target_world.x, target_world.y, target_world.z];

    // Alt + LMB, Alt + RMB, or HUD `[PING]` cursor + LMB sends a tactical PingLocation beacon!
    let lmb_ping =
        mouse_buttons.just_pressed(MouseButton::Left) && (alt_held || interaction.ping_cursor_mode);
    let rmb_ping = mouse_buttons.just_pressed(MouseButton::Right) && alt_held;
    if (lmb_ping || rmb_ping) && !interaction.replay_active && !interaction.spectator_mode_active {
        sim_world.push_command(GameCommand::PingLocation {
            player_id: pid,
            position: pos,
        });
        interaction.push_radar_event(pos);
        if !shift_held {
            interaction.ping_cursor_mode = false;
        }
        if let Some(snd) = sound_res {
            snd.trigger(SoundEffectKind::EvaAlert);
        }
        interaction.push_eva_alert(format!(
            "EVA: Tactical radar ping at ({:.0}, {:.0})!",
            target_world.x, target_world.z
        ));
        return;
    }

    if mouse_buttons.pressed(MouseButton::Left) && !alt_held && !interaction.ping_cursor_mode {
        if let Ok((_, _, mut rig)) = camera_q.get_single_mut() {
            rig.jump_to(target_world);
        }
    }
    if !mouse_buttons.just_pressed(MouseButton::Right)
        || interaction.replay_active
        || interaction.spectator_mode_active
        || interaction.selected_ids.is_empty()
    {
        return;
    }

    let owned_units: Vec<u64> = interaction
        .selected_ids
        .iter()
        .copied()
        .filter(|id| {
            sim_world.entities.get(id).is_some_and(|e| {
                e.player_id == pid
                    && e.transported_by.is_none()
                    && matches!(e.kind, EntityKind::Unit { .. })
            })
        })
        .collect();
    let owned_buildings: Vec<u64> = interaction
        .selected_ids
        .iter()
        .copied()
        .filter(|id| {
            sim_world.entities.get(id).is_some_and(|e| {
                e.player_id == pid && matches!(e.kind, EntityKind::Building { .. })
            })
        })
        .collect();

    if !owned_units.is_empty() {
        let is_atk = ctrl_held || interaction.attack_move_mode;
        if let Some(snd) = sound_res {
            if let Some(first_uid) = owned_units.iter().find_map(|eid| {
                sim_world.entities.get(eid).and_then(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => Some(unit_id.as_str()),
                    _ => None,
                })
            }) {
                let cat = if is_atk {
                    UnitVoiceCategory::Attack
                } else {
                    UnitVoiceCategory::Move
                };
                snd.trigger_unit_ack(first_uid, cat);
            }
        }
        let cmd = if shift_held {
            let order = if ctrl_held {
                QueuedOrder::AttackGround { target_pos: pos }
            } else if interaction.attack_move_mode {
                QueuedOrder::AttackMove {
                    target_pos: pos,
                    facing_radians: None,
                }
            } else if interaction.patrol_mode {
                QueuedOrder::Patrol { target_pos: pos }
            } else {
                QueuedOrder::Move {
                    target_pos: pos,
                    facing_radians: None,
                }
            };
            GameCommand::EnqueueOrder {
                player_id: pid,
                entity_ids: owned_units,
                order,
            }
        } else if ctrl_held {
            GameCommand::AttackGround {
                player_id: pid,
                attacker_ids: owned_units,
                target_pos: pos,
            }
        } else if interaction.attack_move_mode {
            interaction.attack_move_mode = false;
            GameCommand::AttackMove {
                player_id: pid,
                entity_ids: owned_units,
                target_pos: pos,
            }
        } else if interaction.patrol_mode {
            interaction.patrol_mode = false;
            GameCommand::PatrolUnits {
                player_id: pid,
                entity_ids: owned_units,
                target_pos: pos,
            }
        } else {
            GameCommand::MoveUnits {
                player_id: pid,
                entity_ids: owned_units,
                target_pos: pos,
            }
        };
        sim_world.push_command(cmd);
        interaction.pending_command_rings.push((
            Vec3::new(target_world.x, target_world.y + 0.08, target_world.z),
            Color::srgb(0.20, 0.95, 0.35),
        ));
    } else if !owned_buildings.is_empty() {
        if shift_held {
            sim_world.push_command(GameCommand::QueueRallyPoint {
                player_id: pid,
                building_ids: owned_buildings,
                rally_pos: pos,
            });
        } else {
            sim_world.push_command(GameCommand::SetRallyPoint {
                player_id: pid,
                building_ids: owned_buildings,
                rally_pos: pos,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fit_radar_box_dimensions;
    use super::minimap_render::*;

    #[test]
    fn radar_base_terrain_is_100_percent_opaque_and_fits_aspect_ratio() {
        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(
            rts_sdk::loader::LoadedMod::resolve_mod_dir("rust-ra2"),
        )
        .expect("load rust-ra2");
        let world = rts_simulation::world::SimWorld::from_mod(&loaded);
        let rgba = rebuild_static_radar_terrain_256(&loaded.entry_map, &world.grid);
        assert_eq!(rgba.len(), 256 * 256 * 4);
        assert!(
            rgba.chunks_exact(4).all(|px| px[3] == 255),
            "Every base terrain radar pixel MUST have alpha == 255 so radar.png never bleeds through"
        );

        let (sq_w, sq_h) = fit_radar_box_dimensions(64, 64, 192.0, 138.0);
        assert!((sq_w - 138.0).abs() < 0.01 && (sq_h - 138.0).abs() < 0.01);

        let rig = crate::camera::RtsCameraRig::new(128.0, 128.0);
        let trap =
            compute_camera_trapezoid_pixels(None, &rig, &world.grid, false, 128.0, 128.0, 256, 256);
        let top_span = (trap[1].0 - trap[0].0).abs();
        let bot_span = (trap[2].0 - trap[3].0).abs();
        assert!(
            top_span >= bot_span,
            "Perspective camera trapezoid far (top) edge ({top_span}) should be >= near (bottom) edge ({bot_span})"
        );
    }
}
