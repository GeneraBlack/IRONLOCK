use crate::input::{AppPhase, ClientInteractionState};
use crate::ui::locale::ui_t;
use crate::ui::screens::post_game_graphs::{
    create_blank_post_game_graph_image, rasterize_post_game_telemetry_graphs,
};
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::{PostGameGraphImage, SidebarAction};
use crate::{ModResource, NetRuntimeResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Component)]
pub struct ReplayObserverOverlayRoot;

#[derive(Resource, Default)]
pub struct ReplayHudRenderCache {
    pub last_sig: String,
    pub last_graph_tick: u64,
}

pub fn setup_replay_observer_hud(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let graph_handle = images.add(create_blank_post_game_graph_image());
    commands.insert_resource(PostGameGraphImage(graph_handle));
    commands.insert_resource(ReplayHudRenderCache::default());

    commands.spawn((
        ReplayObserverOverlayRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        PickingBehavior::IGNORE,
        ZIndex(26),
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn update_replay_observer_hud(
    mut commands: Commands,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    net_res: Res<NetRuntimeResource>,
    interaction: Res<ClientInteractionState>,
    mut ui_blocker: ResMut<crate::input::UiInputBlocker>,
    mut cache: ResMut<ReplayHudRenderCache>,
    graph_img_res: Option<Res<PostGameGraphImage>>,
    mut images: ResMut<Assets<Image>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    root_q: Query<Entity, With<ReplayObserverOverlayRoot>>,
) {
    // Keep Post-Game 4-quadrant RGBA8 telemetry graph up to date whenever Post-Game Score is open
    if interaction.menu_open
        && interaction.active_menu_screen == crate::input::MenuScreen::PostGameScore
        && cache.last_graph_tick != sim_res.0.tick.wrapping_add(1)
    {
        if let Some(handle_res) = &graph_img_res {
            if let Some(img) = images.get_mut(&handle_res.0) {
                rasterize_post_game_telemetry_graphs(&sim_res.0, img);
                cache.last_graph_tick = sim_res.0.tick.wrapping_add(1);
            }
        }
    }

    let Ok(root_ent) = root_q.get_single() else {
        return;
    };

    let show_ingame = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;
    let is_observer =
        show_ingame && (interaction.replay_active || interaction.spectator_mode_active);
    let show_postgame_pill =
        show_ingame && sim_res.0.winner_player_id.is_some() && !interaction.replay_active;
    let show_reconnect = show_ingame && interaction.reconnect_in_progress;
    let desync_text = interaction.desync_banner_text.as_deref().unwrap_or("");
    let show_desync = show_ingame && !desync_text.is_empty();
    let show_chat_bar = show_ingame && interaction.chat_input_open;

    // Prevent clicks over the bottom replay transport bar or top telemetry box from leaking to 3D terrain
    if is_observer {
        if let Ok(win) = window_q.get_single() {
            if let Some(cur) = win.cursor_position() {
                let over_bottom_bar = cur.y >= win.height() - 186.0
                    && cur.y <= win.height() - 94.0
                    && cur.x >= 10.0
                    && cur.x <= win.width() - 272.0;
                let over_top_strip = cur.y >= 70.0 && cur.y <= 192.0 && cur.x <= 566.0;
                if over_bottom_bar || over_top_strip {
                    ui_blocker.over_player_hud = true;
                }
            }
        }
    }

    let replay_final_tick = net_res
        .active_replay_playback
        .as_ref()
        .map(|r| r.final_tick)
        .unwrap_or(sim_res.0.tick.max(1));

    let sig = format!(
        "obs:{is_observer}|pg:{show_postgame_pill}|rec:{show_reconnect}:{}|ds:{show_desync}:{desync_text}|chat:{show_chat_bar}:{}:{}|t:{}|ft:{replay_final_tick}|p:{}|spd:{:.1}|pov:{}:{}|lang:{}",
        interaction.reconnect_attempts,
        interaction.chat_team_only,
        interaction.chat_input_buffer,
        sim_res.0.tick / 10,
        interaction.game_paused,
        interaction.sim_speed_multiplier,
        interaction.spectator_all_vision,
        interaction.observer_pov_player_id,
        interaction.language,
    );

    if cache.last_sig == sig {
        return;
    }
    cache.last_sig = sig;
    commands.entity(root_ent).despawn_descendants();

    if !is_observer && !show_postgame_pill && !show_reconnect && !show_desync && !show_chat_bar {
        return;
    }

    let world = &sim_res.0;
    let loaded = &mod_res.0;

    commands.entity(root_ent).with_children(|root| {
        // 1. Desync Forensics HUD Alert Banner
        if show_desync {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(40.0),
                    left: Val::Px(260.0),
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.45, 0.06, 0.08, 0.95)),
                BorderColor(Color::srgb(1.0, 0.30, 0.25)),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(sanitize_ui_text(desync_text)),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.88, 0.84)),
                ));
            });
        }

        // 2. Auto-Reconnect Overlay
        if show_reconnect {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(140.0),
                    left: Val::Px(340.0),
                    width: Val::Px(460.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.09, 0.16, 0.96)),
                BorderColor(Color::srgb(0.98, 0.72, 0.24)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(format!(
                        "CONNECTION INTERRUPTED — AUTO-RECONNECT ATTEMPT #{}",
                        interaction.reconnect_attempts.max(1)
                    )),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.84, 0.40)),
                ));
                card.spawn((
                    Text::new(sanitize_ui_text(&interaction.net_status_line)),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.92, 0.98)),
                ));
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|r| {
                    spawn_styled_button(
                        r,
                        "RETRY RECONNECT NOW",
                        SidebarAction::TriggerHotReconnect,
                        Color::srgb(0.20, 0.44, 0.30),
                        Val::Percent(55.0),
                    );
                    spawn_styled_button(
                        r,
                        "ABORT TO MAIN MENU",
                        SidebarAction::ReturnToMainMenu,
                        Color::srgb(0.46, 0.16, 0.18),
                        Val::Percent(45.0),
                    );
                });
            });
        }

        // 3. Re-Open Post-Game Score Pill (when inspecting battlefield after Victory/Defeat)
        if show_postgame_pill {
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Px(42.0),
                right: Val::Px(280.0),
                width: Val::Px(220.0),
                ..default()
            })
            .with_children(|p| {
                spawn_styled_button(
                    p,
                    "OPEN POST-GAME SCORE & GRAPHS",
                    SidebarAction::OpenPostGameScoreScreen,
                    Color::srgb(0.24, 0.48, 0.30),
                    Val::Percent(100.0),
                );
            });
        }

        // 4. In-Game [Enter] / [Shift+Enter] All/Team Chat Input Bar
        if show_chat_bar {
            let chan = if interaction.chat_team_only {
                "[TEAM CHAT]"
            } else {
                "[ALL CHAT]"
            };
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(96.0),
                    left: Val::Px(24.0),
                    width: Val::Px(540.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.07, 0.13, 0.94)),
                BorderColor(Color::srgb(0.42, 0.78, 0.98)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Text::new(sanitize_ui_text(&format!(
                        "{chan}: {}|   (Enter: Send, Esc: Cancel)",
                        interaction.chat_input_buffer
                    ))),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.97, 1.0)),
                ));
            });
        }

        // 5. Dedicated 3D Replay & Live Spectator Observer HUD
        if is_observer {
            // Top-Left Live Player Comparison Bars & Active Production Summary (shifted cleanly below EVA banner)
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(72.0),
                    left: Val::Px(12.0),
                    width: Val::Px(550.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.90)),
                BorderColor(Color::srgb(0.36, 0.62, 0.88)),
            ))
            .with_children(|strip| {
                strip.spawn((
                    Text::new("OBSERVER TELEMETRY — ECONOMY / POWER / ARMY / ACTIVE PRODUCTION"),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.86, 0.44)),
                ));
                for (&pid, econ) in &world.players {
                    if pid == 0 {
                        continue;
                    }
                    let army_cnt = world
                        .entities
                        .values()
                        .filter(|e| e.player_id == pid && e.health > 0.0)
                        .count();
                    let active_prod = econ
                        .building_queue
                        .as_ref()
                        .map(|(bid, _, _)| bid.as_str())
                        .or_else(|| {
                            world
                                .entities
                                .values()
                                .filter(|e| e.player_id == pid)
                                .find_map(|e| {
                                    e.production_queue.first().map(|q| q.unit_id.as_str())
                                })
                        })
                        .unwrap_or("Idle");
                    let apm = world
                        .telemetry
                        .player_samples
                        .get(&pid)
                        .and_then(|s| s.last().map(|x| x.apm))
                        .unwrap_or(0);
                    let net_p = econ.power_produced - econ.power_consumed;
                    strip.spawn((
                        Text::new(sanitize_ui_text(&format!(
                            "P{pid} [{}]  ${:<6} | Pwr {:+4} | Units {:<3} | APM {:<3} | Prod: {}",
                            econ.faction, econ.credits, net_p, army_cnt, apm, active_prod
                        ))),
                        TextFont {
                            font_size: 10.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.94, 0.98)),
                    ));
                }
            });

            // Bottom-Left Replay / Spectator Transport, Timeline Scrubber & POV Selector (above unit selection bar)
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(98.0),
                    left: Val::Px(12.0),
                    width: Val::Px(980.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.05, 0.10, 0.93)),
                BorderColor(Color::srgb(0.42, 0.68, 0.94)),
            ))
            .with_children(|bar| {
                // Row 1: Play/Pause, -10s/+10s, Scrubber Checkpoints, Speed 0.5x..8x
                bar.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|r1| {
                    spawn_styled_button(
                        r1,
                        if interaction.game_paused {
                            "PLAY"
                        } else {
                            "PAUSE"
                        },
                        SidebarAction::TogglePause,
                        Color::srgb(0.18, 0.44, 0.30),
                        Val::Px(64.0),
                    );
                    if interaction.replay_active {
                        spawn_styled_button(
                            r1,
                            "-10s",
                            SidebarAction::SeekReplayRelative(-200),
                            Color::srgb(0.20, 0.30, 0.44),
                            Val::Px(48.0),
                        );
                        spawn_styled_button(
                            r1,
                            "+10s",
                            SidebarAction::SeekReplayRelative(200),
                            Color::srgb(0.20, 0.30, 0.44),
                            Val::Px(48.0),
                        );
                        for pct in [0u64, 25, 50, 75, 100] {
                            let target_t = (replay_final_tick.saturating_mul(pct)) / 100;
                            spawn_styled_button(
                                r1,
                                &format!("{pct}%"),
                                SidebarAction::SeekReplayAbsoluteTick(target_t),
                                Color::srgb(0.16, 0.26, 0.38),
                                Val::Px(44.0),
                            );
                        }
                        for (tenths, lbl) in [
                            (5u32, "0.5x"),
                            (10, "1x"),
                            (20, "2x"),
                            (40, "4x"),
                            (80, "8x"),
                        ] {
                            let active_spd = ((interaction.sim_speed_multiplier * 10.0).round()
                                as u32)
                                == tenths;
                            spawn_styled_button(
                                r1,
                                lbl,
                                SidebarAction::SetReplaySpeedTenths(tenths),
                                if active_spd {
                                    Color::srgb(0.26, 0.52, 0.34)
                                } else {
                                    Color::srgb(0.16, 0.22, 0.32)
                                },
                                Val::Px(46.0),
                            );
                        }
                    }
                    let cur_s = world.tick / 20;
                    let tot_s = replay_final_tick / 20;
                    r1.spawn((
                        Text::new(format!(
                            " {:02}:{:02}/{:02}:{:02} (T{})",
                            cur_s / 60,
                            cur_s % 60,
                            tot_s / 60,
                            tot_s % 60,
                            world.tick
                        )),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.92, 0.96, 1.0)),
                    ));
                });

                // Row 2: Scrubber Progress Bar + POV Selector (All Players 255 / P1..P8)
                bar.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|r2| {
                    let all_lbl = ui_t(loaded, &interaction, "ui.replay.all_vision");
                    let all_active = interaction.spectator_all_vision
                        || interaction.observer_pov_player_id == 255;
                    spawn_styled_button(
                        r2,
                        &all_lbl,
                        SidebarAction::SetReplayObserverPov(255),
                        if all_active {
                            Color::srgb(0.24, 0.50, 0.34)
                        } else {
                            Color::srgb(0.16, 0.24, 0.34)
                        },
                        Val::Px(190.0),
                    );
                    for (&pid, econ) in &world.players {
                        if pid == 0 {
                            continue;
                        }
                        let sel = !all_active && interaction.observer_pov_player_id == pid;
                        spawn_styled_button(
                            r2,
                            &format!("POV P{pid} ({})", econ.faction),
                            SidebarAction::SetReplayObserverPov(pid),
                            if sel {
                                Color::srgb(0.24, 0.46, 0.64)
                            } else {
                                Color::srgb(0.16, 0.22, 0.34)
                            },
                            Val::Px(115.0),
                        );
                    }
                    spawn_styled_button(
                        r2,
                        "EXIT REPLAY",
                        SidebarAction::ReturnToMainMenu,
                        Color::srgb(0.46, 0.16, 0.18),
                        Val::Px(105.0),
                    );
                });
            });
        }
    });
}
