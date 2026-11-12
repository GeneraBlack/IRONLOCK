use crate::input::{AppPhase, ClientInteractionState, UiInputBlocker};
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::widgets::ui_node_logical_rect;
use crate::ui::SidebarAction;
use crate::SimWorldResource;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::schema::{ObjectiveStatus, ObjectiveTier};

#[derive(Component)]
pub struct ObjectivesHudRoot;

#[derive(Component)]
pub struct ObjectivesHudContentContainer;

#[derive(Default)]
pub struct ObjectivesHudSigCache(pub String);

pub fn spawn_objectives_hud(commands: &mut Commands) {
    commands
        .spawn((
            ObjectivesHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(74.0),
                width: Val::Px(336.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.88)),
            BorderColor(Color::srgba(0.32, 0.62, 0.90, 0.82)),
            ZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                ObjectivesHudContentContainer,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    ..default()
                },
            ));
        });
}

fn tier_badge_and_color(lang: &str, tier: ObjectiveTier) -> (&'static str, Color) {
    match tier {
        ObjectiveTier::Primary => (
            if lang == "de" { "PRIMÄR" } else { "PRIMARY" },
            Color::srgb(0.98, 0.84, 0.34),
        ),
        ObjectiveTier::Secondary => (
            if lang == "de" {
                "SEKUNDÄR"
            } else {
                "SECONDARY"
            },
            Color::srgb(0.52, 0.84, 1.0),
        ),
        ObjectiveTier::Bonus => ("BONUS", Color::srgb(0.76, 0.62, 1.0)),
    }
}

fn status_tag_and_color(status: ObjectiveStatus) -> (&'static str, Color) {
    match status {
        ObjectiveStatus::Completed => ("[OK]", Color::srgb(0.36, 0.95, 0.54)),
        ObjectiveStatus::Failed => ("[FAIL]", Color::srgb(0.98, 0.38, 0.38)),
        ObjectiveStatus::Active => ("[>>]", Color::srgb(0.96, 0.92, 0.68)),
        ObjectiveStatus::Pending => ("[..]", Color::srgb(0.62, 0.70, 0.82)),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_objectives_hud(
    mut commands: Commands,
    interaction: Res<ClientInteractionState>,
    sim_res: Res<SimWorldResource>,
    mut ui_blocker: ResMut<UiInputBlocker>,
    mut sig_cache: Local<ObjectivesHudSigCache>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut root_q: Query<(&mut Node, &ComputedNode, &GlobalTransform), With<ObjectivesHudRoot>>,
    content_q: Query<Entity, With<ObjectivesHudContentContainer>>,
) {
    let Ok((mut root_node, computed, gt)) = root_q.get_single_mut() else {
        return;
    };

    let world = &sim_res.0;
    let has_content = !world.mission_objectives.is_empty() || !world.countdown_timers.is_empty();
    let show = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open
        && !interaction.dev_console_open
        && has_content;

    root_node.display = if show { Display::Flex } else { Display::None };
    if !show {
        return;
    }

    if let Ok(window) = window_q.get_single() {
        if let Some(cursor) = window.cursor_position() {
            let rect = ui_node_logical_rect(computed, gt, window.scale_factor());
            if rect.width() > 2.0 && rect.height() > 2.0 && rect.contains(cursor) {
                ui_blocker.over_player_hud = true;
            }
        }
    }

    let timers_sig: String = world
        .countdown_timers
        .iter()
        .map(|t| format!("{}:{}", t.timer_id, t.format_mm_ss(world.dt)))
        .collect::<Vec<_>>()
        .join("|");
    let objs_sig: String = world
        .mission_objectives
        .iter()
        .map(|o| {
            format!(
                "{}:{:?}:{}/{}:{:?}",
                o.def.id, o.status, o.current_count, o.def.target_count, o.def.marker_tile
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let sig = format!(
        "col:{}|lang:{}|t:{}|o:{}",
        interaction.objectives_hud_collapsed, interaction.language, timers_sig, objs_sig
    );

    if sig_cache.0 == sig {
        return;
    }
    sig_cache.0 = sig;

    let Ok(content_ent) = content_q.get_single() else {
        return;
    };
    commands.entity(content_ent).despawn_descendants();

    let lang = interaction.language.as_str();
    let collapsed = interaction.objectives_hud_collapsed;
    let timers = world.countdown_timers.clone();
    let objectives = world.mission_objectives.clone();
    let sim_dt = world.dt;

    commands.entity(content_ent).with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new(sanitize_ui_text(tr(lang, "hud_objectives_title"))),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.90, 0.56)),
                ));
                spawn_styled_button(
                    hdr,
                    if collapsed { "[+]" } else { "[-]" },
                    SidebarAction::ToggleObjectivesHudCollapse,
                    Color::srgb(0.18, 0.28, 0.42),
                    Val::Auto,
                );
            });

        // Countdown timers always stay visible even when objective list is collapsed!
        for timer in &timers {
            let mm_ss = timer.format_mm_ss(sim_dt);
            let urgent = timer.remaining_ticks <= 20 * 30;
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(if urgent {
                        Color::srgba(0.36, 0.08, 0.10, 0.92)
                    } else {
                        Color::srgba(0.08, 0.14, 0.24, 0.92)
                    }),
                    BorderColor(if urgent {
                        Color::srgb(0.98, 0.36, 0.36)
                    } else {
                        Color::srgb(0.36, 0.68, 0.96)
                    }),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(sanitize_ui_text(&timer.label)),
                        TextFont {
                            font_size: 10.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.90, 0.95, 1.0)),
                    ));
                    row.spawn((
                        Text::new(mm_ss),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(if urgent {
                            Color::srgb(1.0, 0.44, 0.44)
                        } else {
                            Color::srgb(0.98, 0.90, 0.42)
                        }),
                    ));
                });
        }

        if collapsed {
            return;
        }

        for obj in &objectives {
            if obj.status == ObjectiveStatus::Pending {
                continue;
            }
            let (tier_lbl, tier_col) = tier_badge_and_color(lang, obj.def.tier);
            let (st_lbl, st_col) = status_tag_and_color(obj.status);
            let counter_str = if obj.def.target_count > 1 {
                format!(
                    " ({}/{})",
                    obj.current_count.min(obj.def.target_count),
                    obj.def.target_count
                )
            } else {
                String::new()
            };

            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                        border: UiRect::left(Val::Px(2.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.10, 0.16, 0.85)),
                    BorderColor(tier_col),
                ))
                .with_children(|card| {
                    card.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|top_row| {
                        top_row.spawn((
                            Text::new(sanitize_ui_text(&format!(
                                "{st_lbl} [{tier_lbl}] {}{counter_str}",
                                obj.def.title
                            ))),
                            TextFont {
                                font_size: 10.5,
                                ..default()
                            },
                            TextColor(st_col),
                        ));
                        if let Some(tile) = obj.def.marker_tile {
                            spawn_styled_button(
                                top_row,
                                tr(lang, "hud_obj_focus"),
                                SidebarAction::JumpCameraToTile(tile),
                                Color::srgb(0.18, 0.36, 0.52),
                                Val::Auto,
                            );
                        }
                    });

                    if let Some(desc) = obj
                        .def
                        .description
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                    {
                        if obj.status == ObjectiveStatus::Active {
                            card.spawn((
                                Text::new(sanitize_ui_text(desc)),
                                TextFont {
                                    font_size: 9.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.74, 0.82, 0.92)),
                            ));
                        }
                    }
                });
        }
    });
}
