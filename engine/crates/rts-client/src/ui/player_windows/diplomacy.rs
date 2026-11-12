use crate::input::ClientInteractionState;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

pub const QUICK_TAUNTS: &[&str] = &[
    "Need backup at my coordinates!",
    "Launching main assault now!",
    "Resource tribute dispatched, Commander.",
    "Well played, Commander!",
];

pub fn spawn_diplomacy_window(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &ClientInteractionState,
) {
    let local_pid = interaction.local_player_id;
    let local_credits = world
        .players
        .get(&local_pid)
        .map(|p| p.credits)
        .unwrap_or(0);

    parent.spawn((
        Text::new(format!(
            "Commander P{local_pid} Treasury: ${local_credits}  |  Alliance & Resource Tribute Matrix"
        )),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.88, 0.55)),
    ));

    let mut player_ids: Vec<u8> = world.players.keys().copied().filter(|&p| p > 0).collect();
    player_ids.sort();

    for pid in player_ids {
        let Some(pstate) = world.players.get(&pid) else {
            continue;
        };
        let fac_def = loaded.factions.get(&pstate.faction);
        let fac_name = fac_def
            .map(|f| f.name.as_str())
            .unwrap_or(pstate.faction.as_str());
        let rgb = fac_def.map(|f| f.color).unwrap_or([0.45, 0.65, 0.90]);
        let allied = pid == local_pid || !world.are_hostile(local_pid, pid);

        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.94)),
                BorderColor(Color::srgb(rgb[0], rgb[1], rgb[2])),
            ))
            .with_children(|row| {
                // Faction swatch + Player Name + Stance
                row.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|left| {
                    left.spawn((
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(rgb[0], rgb[1], rgb[2])),
                        BorderColor(Color::WHITE),
                    ));
                    let stance_txt = if pid == local_pid {
                        "YOU"
                    } else if allied {
                        "ALLIED"
                    } else {
                        "HOSTILE"
                    };
                    left.spawn((
                        Text::new(sanitize_ui_text(&format!(
                            "P{pid} — {fac_name}  [{stance_txt}]"
                        ))),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(if allied {
                            Color::srgb(0.62, 0.96, 0.72)
                        } else {
                            Color::srgb(0.98, 0.62, 0.58)
                        }),
                    ));
                });

                if pid != local_pid {
                    row.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|btns| {
                        let toggle_lbl = if allied { "BREAK PACT" } else { "OFFER PACT" };
                        spawn_styled_button(
                            btns,
                            toggle_lbl,
                            SidebarAction::ToggleAllianceWithPlayer(pid),
                            if allied {
                                Color::srgb(0.44, 0.20, 0.18)
                            } else {
                                Color::srgb(0.18, 0.40, 0.28)
                            },
                            Val::Auto,
                        );
                        spawn_styled_button(
                            btns,
                            "+$500",
                            SidebarAction::TransferCreditsToPlayer(pid, 500),
                            Color::srgb(0.22, 0.34, 0.48),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            btns,
                            "+$1000",
                            SidebarAction::TransferCreditsToPlayer(pid, 1000),
                            Color::srgb(0.28, 0.38, 0.18),
                            Val::Auto,
                        );
                    });
                }
            });
    }

    // Quick Tactical Radio / Taunt Transmission Bar
    parent.spawn((
        Text::new("Quick Radio / Taunt Transmissions (Press [Enter] in-game for custom chat):"),
        TextFont {
            font_size: 11.5,
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.85, 0.96)),
    ));

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(5.0),
            ..default()
        })
        .with_children(|grid| {
            for (idx, msg) in QUICK_TAUNTS.iter().enumerate() {
                spawn_styled_button(
                    grid,
                    msg,
                    SidebarAction::SendQuickTaunt(idx),
                    Color::srgb(0.18, 0.28, 0.42),
                    Val::Percent(48.5),
                );
            }
        });
}
