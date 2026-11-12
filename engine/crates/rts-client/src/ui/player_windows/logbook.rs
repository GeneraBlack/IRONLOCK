use crate::input::ClientInteractionState;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ObjectiveStatus, ObjectiveTier};
use rts_simulation::world::SimWorld;

pub fn spawn_logbook_window(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &ClientInteractionState,
) {
    parent.spawn((
        Text::new("PRIMARY, SECONDARY & BONUS TACTICAL OBJECTIVES"),
        TextFont {
            font_size: 12.5,
            ..default()
        },
        TextColor(Color::srgb(0.98, 0.86, 0.42)),
    ));

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.16, 0.95)),
            BorderColor(Color::srgb(0.28, 0.44, 0.64)),
        ))
        .with_children(|obj_box| {
            if !world.mission_objectives.is_empty() {
                for obj in &world.mission_objectives {
                    let tier_lbl = match obj.def.tier {
                        ObjectiveTier::Primary => "Primary",
                        ObjectiveTier::Secondary => "Secondary",
                        ObjectiveTier::Bonus => "Bonus",
                    };
                    let (st_lbl, st_col) = match obj.status {
                        ObjectiveStatus::Completed => {
                            ("[COMPLETED]", Color::srgb(0.45, 0.96, 0.58))
                        }
                        ObjectiveStatus::Failed => ("[FAILED]", Color::srgb(0.98, 0.45, 0.42)),
                        ObjectiveStatus::Active => ("[ACTIVE]", Color::srgb(0.98, 0.84, 0.38)),
                        ObjectiveStatus::Pending => ("[PENDING]", Color::srgb(0.62, 0.72, 0.84)),
                    };
                    let counter = if obj.def.target_count > 1 {
                        format!(
                            " ({}/{})",
                            obj.current_count.min(obj.def.target_count),
                            obj.def.target_count
                        )
                    } else {
                        String::new()
                    };

                    obj_box
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Text::new(sanitize_ui_text(&format!(
                                    "{st_lbl} {tier_lbl}: {}{counter}",
                                    obj.def.title
                                ))),
                                TextFont {
                                    font_size: 11.5,
                                    ..default()
                                },
                                TextColor(st_col),
                            ));
                            if let Some(tile) = obj.def.marker_tile {
                                spawn_styled_button(
                                    row,
                                    "FOCUS",
                                    SidebarAction::JumpCameraToTile(tile),
                                    Color::srgb(0.18, 0.36, 0.52),
                                    Val::Auto,
                                );
                            }
                        });
                }
            } else {
                let primary_status = match world.winner_player_id {
                    Some(w) if w == interaction.local_player_id => {
                        ("[COMPLETED]", Color::srgb(0.45, 0.96, 0.58))
                    }
                    Some(_) => ("[FAILED]", Color::srgb(0.98, 0.45, 0.42)),
                    None => ("[ACTIVE]", Color::srgb(0.98, 0.84, 0.38)),
                };
                let primary_obj = world
                    .mission_objective
                    .clone()
                    .or_else(|| {
                        interaction
                            .active_campaign_id
                            .as_ref()
                            .and_then(|cid| loaded.campaigns.get(cid))
                            .and_then(|c| {
                                interaction
                                    .active_mission_id
                                    .as_ref()
                                    .and_then(|mid| c.missions.iter().find(|m| &m.id == mid))
                            })
                            .map(|m| m.briefing.clone())
                    })
                    .unwrap_or_else(|| {
                        "Destroy all hostile command structures and secure sector dominance."
                            .to_string()
                    });
                obj_box.spawn((
                    Text::new(sanitize_ui_text(&format!(
                        "{} Primary: {}",
                        primary_status.0, primary_obj
                    ))),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(primary_status.1),
                ));
            }
        });

    parent.spawn((
        Text::new(format!(
            "EVA & RADIO TRANSMISSION LOG ({} entries — Scroll with Mouse Wheel):",
            interaction.eva_log_archive.len()
        )),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.88, 0.98)),
    ));

    let max_scroll = (interaction.eva_log_archive.len() as f32 * 22.0 - 220.0).max(0.0);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(240.0),
                overflow: Overflow::clip_y(),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.08, 0.96)),
            BorderColor(Color::srgb(0.24, 0.34, 0.48)),
            UiScrollContainer::new(max_scroll),
        ))
        .with_children(|vp| {
            vp.spawn((
                UiScrollContent,
                Node {
                    width: Val::Percent(100.0),
                    position_type: PositionType::Relative,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|list| {
                for (seq, msg) in interaction.eva_log_archive.iter().rev() {
                    list.spawn((
                        Text::new(sanitize_ui_text(&format!("[#{seq:03}] {msg}"))),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.92, 0.88)),
                    ));
                }
            });
        });
}
