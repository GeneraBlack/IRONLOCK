use crate::input::ClientInteractionState;
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CommanderPowerNodeDef, CommanderPowerTreeDef};
use rts_simulation::world::SimWorld;
use std::collections::BTreeSet;

fn resolve_player_commander_tree<'a>(
    loaded: &'a LoadedMod,
    faction_id: &str,
) -> Option<&'a CommanderPowerTreeDef> {
    loaded
        .commander_trees
        .values()
        .find(|t| t.faction.eq_ignore_ascii_case(faction_id))
        .or_else(|| loaded.commander_trees.values().next())
}

pub fn spawn_commander_tree_window(
    body: &mut ChildBuilder,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &ClientInteractionState,
    pid: u8,
    faction_id: &str,
    border_col: Color,
) {
    let lang = interaction.language.as_str();
    let econ = world.players.get(&pid);
    let rank = econ.map(|e| e.commander_rank).unwrap_or(1).max(1);
    let xp = econ.map(|e| e.commander_xp).unwrap_or(0.0);
    let promo_pts = econ.map(|e| e.promotion_points).unwrap_or(0);
    let empty_set: BTreeSet<String> = BTreeSet::new();
    let unlocked = world
        .unlocked_commander_nodes
        .get(&pid)
        .unwrap_or(&empty_set);

    let stars = "*".repeat(rank.min(5) as usize);
    let header_summary = format!(
        "{}: {} ({stars} Rank {rank})  |  XP: {:.0}  |  {}: {promo_pts}",
        tr(lang, "cmd_tree_rank"),
        faction_id.to_uppercase(),
        xp,
        tr(lang, "cmd_tree_points")
    );

    body.spawn((
        Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.5)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.07, 0.12, 0.22, 0.95)),
        BorderColor(border_col),
    ))
    .with_children(|hdr| {
        hdr.spawn((
            Text::new(sanitize_ui_text(&header_summary)),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::srgb(0.98, 0.90, 0.44)),
        ));
    });

    let Some(tree) = resolve_player_commander_tree(loaded, faction_id) else {
        body.spawn((
            Text::new(sanitize_ui_text(tr(lang, "cmd_tree_none"))),
            TextFont {
                font_size: 11.5,
                ..default()
            },
            TextColor(Color::srgb(0.76, 0.82, 0.92)),
        ));
        return;
    };

    body.spawn((
        Text::new(sanitize_ui_text(&format!(
            "DOCTRINE: {}",
            loaded.t(lang, &tree.display_name)
        ))),
        TextFont {
            font_size: 11.5,
            ..default()
        },
        TextColor(Color::srgb(0.64, 0.88, 1.0)),
    ));

    // Group branching nodes by `required_rank` (`1★..5★`)
    for tier_rank in 1..=5u8 {
        let tier_nodes: Vec<&CommanderPowerNodeDef> = tree
            .nodes
            .iter()
            .filter(|n| n.required_rank.clamp(1, 5) == tier_rank)
            .collect();
        if tier_nodes.is_empty() {
            continue;
        }

        let rank_unlocked = rank >= tier_rank;
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(7.0)),
                border: UiRect::left(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.90)),
            BorderColor(if rank_unlocked {
                Color::srgb(0.96, 0.82, 0.34)
            } else {
                Color::srgb(0.32, 0.40, 0.52)
            }),
        ))
        .with_children(|tier_col| {
            tier_col.spawn((
                Text::new(format!(
                    "TIER {tier_rank}* PROMOTIONS {}",
                    if rank_unlocked {
                        "[AVAILABLE]"
                    } else {
                        "[LOCKED]"
                    }
                )),
                TextFont {
                    font_size: 10.5,
                    ..default()
                },
                TextColor(if rank_unlocked {
                    Color::srgb(0.98, 0.88, 0.46)
                } else {
                    Color::srgb(0.60, 0.68, 0.78)
                }),
            ));

            tier_col
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|cards| {
                    for node in tier_nodes {
                        let is_owned = unlocked.contains(&node.id);
                        let prereqs_met =
                            node.prerequisites.iter().all(|req| unlocked.contains(req));
                        let cost = node.cost_points.max(1);
                        let can_unlock =
                            !is_owned && rank_unlocked && prereqs_met && promo_pts >= cost;

                        let mut grants_tags: Vec<String> = Vec::new();
                        if let Some(ref sp) = node.grants_support_power {
                            grants_tags.push(format!("Power: {sp}"));
                        }
                        if let Some(ref upg) = node.grants_upgrade {
                            grants_tags.push(format!("Buff: {upg}"));
                        }
                        if let Some(ref u) = node.unlocks_def_id {
                            grants_tags.push(format!("Unlocks: {u}"));
                        }
                        if !node.prerequisites.is_empty() {
                            grants_tags.push(format!("Req: {}", node.prerequisites.join(", ")));
                        }

                        cards
                            .spawn((
                                Node {
                                    width: Val::Px(220.0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(4.0),
                                    padding: UiRect::all(Val::Px(7.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                BackgroundColor(if is_owned {
                                    Color::srgba(0.12, 0.28, 0.20, 0.95)
                                } else if can_unlock {
                                    Color::srgba(0.10, 0.20, 0.34, 0.95)
                                } else {
                                    Color::srgba(0.06, 0.08, 0.12, 0.88)
                                }),
                                BorderColor(if is_owned {
                                    Color::srgb(0.38, 0.94, 0.58)
                                } else if can_unlock {
                                    Color::srgb(0.46, 0.80, 1.0)
                                } else {
                                    Color::srgba(0.28, 0.36, 0.48, 0.65)
                                }),
                            ))
                            .with_children(|card| {
                                card.spawn((
                                    Text::new(
                                        sanitize_ui_text(loaded.t(lang, &node.display_name))
                                            .replace('★', "*"),
                                    ),
                                    TextFont {
                                        font_size: 11.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.96, 0.94, 0.72)),
                                ));
                                if !node.description.trim().is_empty() {
                                    card.spawn((
                                        Text::new(sanitize_ui_text(
                                            loaded.t(lang, &node.description),
                                        )),
                                        TextFont {
                                            font_size: 9.5,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.78, 0.86, 0.95)),
                                    ));
                                }
                                if !grants_tags.is_empty() {
                                    card.spawn((
                                        Text::new(sanitize_ui_text(&grants_tags.join(" | "))),
                                        TextFont {
                                            font_size: 9.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.56, 0.86, 0.96)),
                                    ));
                                }

                                let (btn_lbl, btn_col) = if is_owned {
                                    (
                                        "ACTIVE [UNLOCKED]".to_string(),
                                        Color::srgb(0.18, 0.46, 0.28),
                                    )
                                } else if can_unlock {
                                    (format!("UNLOCK ({cost} PT)"), Color::srgb(0.24, 0.48, 0.68))
                                } else if !rank_unlocked {
                                    (
                                        format!("REQ RANK {tier_rank}*"),
                                        Color::srgb(0.20, 0.22, 0.28),
                                    )
                                } else if !prereqs_met {
                                    ("REQ PREREQ".to_string(), Color::srgb(0.22, 0.20, 0.28))
                                } else {
                                    (format!("NEED {cost} PT"), Color::srgb(0.26, 0.22, 0.18))
                                };
                                spawn_styled_button(
                                    card,
                                    &btn_lbl,
                                    SidebarAction::UnlockCommanderNode(node.id.clone()),
                                    btn_col,
                                    Val::Percent(100.0),
                                );
                            });
                    }
                });
        });
    }
}
