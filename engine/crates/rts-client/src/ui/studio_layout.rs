use super::*;
use crate::input::{ClientInteractionState, MenuScreen};
use bevy::prelude::*;

/// Converts non-ASCII characters (German Umlauts, tactical badges, and Unicode symbols)
/// into crisp ASCII equivalents so Bevy's built-in `FiraMono-subset` font never renders `▯` missing-glyph boxes.
pub fn sanitize_ui_text(input: &str) -> String {
    let pre = input
        .replace("â€”", "-")
        .replace("â˜…", "[*]")
        .replace("âœ”", "[OK]")
        .replace("ðŸ”’", "[LOCK]")
        .replace("â€¢", "*")
        .replace("ðŸ’¬", "[CHAT]");
    let mut out = String::with_capacity(pre.len() + 16);
    for ch in pre.chars() {
        match ch {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'Ä' => out.push_str("AE"),
            'Ö' => out.push_str("OE"),
            'Ü' => out.push_str("UE"),
            'ß' => out.push_str("ss"),
            '—' | '–' => out.push('-'),
            '×' => out.push('x'),
            '•' => out.push('*'),
            '↔' | '⇄' => out.push_str("<->"),
            '★' => out.push_str("[*]"),
            '✔' => out.push_str("[OK]"),
            '▶' => out.push('>'),
            '◀' => out.push('<'),
            '⟳' | '🔄' => out.push_str("[R] "),
            '🎲' => out.push_str("[RNG] "),
            '🎬' => out.push_str("[FMV] "),
            '📡' => out.push_str("[PIP] "),
            '🎵' => out.push_str("[BGM] "),
            '🔊' => out.push_str("[VOL] "),
            '🎥' => out.push_str("[CAM] "),
            '🌤' | '🌫' => out.push_str("[ENV] "),
            '🔨' | '⚙' => out.push_str("[DEV] "),
            '🔍' => out.push_str("[SCAN] "),
            '🖌' => out.push_str("[EDIT] "),
            '🗺' => out.push_str("[MAP] "),
            '⚖' => out.push_str("[BAL] "),
            '💾' => out.push_str("[SAVE] "),
            '📂' => out.push_str("[LOAD] "),
            '⚠' | '⚡' => out.push_str("[!] "),
            '🔒' => out.push_str("[LOCK]"),
            '💬' => out.push_str("[CHAT]"),
            '🌐' => out.push_str("[NET] "),
            '✎' => out.push_str("[EDIT] "),
            '👁' => out.push_str("[POV] "),
            '⏪' => out.push_str("<<"),
            '⏩' | '⏭' => out.push_str(">>"),
            c if c.is_ascii() => out.push(c),
            _ => {}
        }
    }
    out
}

pub(super) fn spawn_studio_modal(commands: &mut Commands) {
    // Full-Viewport Triple-A Command-Center Canvas (Live 3D Orbital Diorama Stage + Glassmorphic Blades)
    commands
        .spawn((
            StudioModalRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.14)),
        ))
        .with_children(|modal| {
            // Top Glassmorphic Command-Center Deck (shown in Engine Dev Mode [F12]; hidden in RA2 Player Mode)
            modal
                .spawn((
                    StudioTopDeckNode,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(12.0)),
                        row_gap: Val::Px(8.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.07, 0.12, 0.92)),
                    BorderColor(Color::srgb(0.32, 0.58, 0.92)),
                ))
                .with_children(|top_deck| {
                    // 1. Header Row: Brand Title + Quick Resume / Return to Main Menu
                    top_deck
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                StudioHeaderTitleText,
                                Text::new(sanitize_ui_text(
                                    "COMMAND & CONQUER: RED ALERT 2 (IRONLOCK EDITION) - COMMAND CENTER",
                                )),
                                TextFont {
                                    font_size: 13.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.88, 0.48)),
                            ));
                            hdr.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|actions| {
                                spawn_styled_button(
                                    actions,
                                    "DEV MODE [F12]",
                                    SidebarAction::ToggleDevMode,
                                    Color::srgb(0.36, 0.22, 0.14),
                                    Val::Auto,
                                );
                                spawn_styled_button(
                                    actions,
                                    "TACTICAL PAUSE",
                                    SidebarAction::NavigateMenu(MenuScreen::PauseMenu),
                                    Color::srgb(0.28, 0.24, 0.38),
                                    Val::Auto,
                                );
                                spawn_styled_button(
                                    actions,
                                    "▶ ENTER / RESUME LIVE 3D BATTLE [ESC]",
                                    SidebarAction::FinishLoadingEnterGame,
                                    Color::srgb(0.16, 0.48, 0.28),
                                    Val::Auto,
                                );
                            });
                        });

                    // 2. Top Submenu Navigation Tab Bar (7 Primary Screens)
                    top_deck
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|nav| {
                            spawn_styled_button(
                                nav,
                                "1. MAIN MENU",
                                SidebarAction::NavigateMenu(MenuScreen::Root),
                                Color::srgb(0.16, 0.28, 0.46),
                                Val::Percent(14.0),
                            );
                            spawn_styled_button(
                                nav,
                                "2. CAMPAIGN",
                                SidebarAction::NavigateMenu(MenuScreen::Campaign),
                                Color::srgb(0.40, 0.26, 0.14),
                                Val::Percent(14.0),
                            );
                            spawn_styled_button(
                                nav,
                                "3. SKIRMISH",
                                SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                                Color::srgb(0.18, 0.38, 0.26),
                                Val::Percent(16.0),
                            );
                            spawn_styled_button(
                                nav,
                                "4. MULTIPLAYER",
                                SidebarAction::NavigateMenu(MenuScreen::Multiplayer),
                                Color::srgb(0.16, 0.38, 0.48),
                                Val::Percent(15.0),
                            );
                            spawn_styled_button(
                                nav,
                                "5. REPLAYS & ARCHIVE",
                                SidebarAction::NavigateMenu(MenuScreen::Theater),
                                Color::srgb(0.32, 0.24, 0.44),
                                Val::Percent(15.0),
                            );
                            nav.spawn((
                                Button,
                                SidebarAction::NavigateMenu(MenuScreen::Studio),
                                ButtonVisualStyle {
                                    normal: Color::srgb(0.24, 0.30, 0.48),
                                    hovered: Color::srgb(0.40, 0.46, 0.64),
                                    pressed: Color::srgb(0.17, 0.22, 0.35),
                                },
                                Node {
                                    width: Val::Percent(14.0),
                                    padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.24, 0.30, 0.48)),
                                BorderColor(Color::srgba(0.85, 0.92, 1.0, 0.28)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    StudioTab6ButtonText,
                                    Text::new("6. UNIT ENCYCLOPEDIA"),
                                    TextFont {
                                        font_size: 11.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                            spawn_styled_button(
                                nav,
                                "7. OPTIONS",
                                SidebarAction::NavigateMenu(MenuScreen::Settings),
                                Color::srgb(0.24, 0.28, 0.36),
                                Val::Percent(12.0),
                            );
                        });

                    // 3. Status & EVA Command Output Banner
                    top_deck
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.12, 0.19, 0.92)),
                            BorderColor(Color::srgb(0.28, 0.44, 0.66)),
                        ))
                        .with_children(|banner| {
                            banner.spawn((
                                StudioStatusText,
                                Text::new("Ready."),
                                TextFont {
                                    font_size: 11.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.55, 0.95, 0.68)),
                            ));
                        });
                });

            // 4. Dynamic Submenu Content Viewport (populated by `menu_screens.rs` based on `active_menu_screen` & `app_phase`)
            modal.spawn((
                StudioModListContainer,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(14.0),
                    ..default()
                },
            ));
        });
}

pub fn spawn_styled_button(
    parent: &mut ChildBuilder,
    label: &str,
    action: SidebarAction,
    base_color: Color,
    width: Val,
) {
    spawn_icon_styled_button(parent, label, None, action, base_color, width);
}

pub fn spawn_icon_styled_button(
    parent: &mut ChildBuilder,
    label: &str,
    icon_handle: Option<Handle<Image>>,
    action: SidebarAction,
    base_color: Color,
    width: Val,
) {
    let srgba = base_color.to_srgba();
    let hovered = Color::srgb(
        (srgba.red + 0.16).min(1.0),
        (srgba.green + 0.16).min(1.0),
        (srgba.blue + 0.16).min(1.0),
    );
    let pressed = Color::srgb(
        (srgba.red * 0.72).max(0.0),
        (srgba.green * 0.72).max(0.0),
        (srgba.blue * 0.72).max(0.0),
    );

    parent
        .spawn((
            Button,
            action,
            ButtonVisualStyle {
                normal: base_color,
                hovered,
                pressed,
            },
            Node {
                width,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                justify_content: if icon_handle.is_some() {
                    JustifyContent::FlexStart
                } else {
                    JustifyContent::Center
                },
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(base_color),
            BorderColor(Color::srgba(0.85, 0.92, 1.0, 0.28)),
        ))
        .with_children(|btn| {
            if let Some(img) = icon_handle {
                btn.spawn((
                    ImageNode::new(img),
                    Node {
                        width: Val::Px(28.0),
                        height: Val::Px(22.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(Color::srgb(0.95, 0.85, 0.45)),
                ));
            }
            btn.spawn((
                Text::new(sanitize_ui_text(label)),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_cameo_grid_button(
    parent: &mut ChildBuilder,
    short_title: &str,
    cost_label: &str,
    status_badge: &str,
    icon_handle: Option<Handle<Image>>,
    action: SidebarAction,
    base_color: Color,
    is_highlighted: bool,
) {
    let border_col = if is_highlighted {
        Color::srgb(1.0, 0.90, 0.30)
    } else {
        Color::srgba(0.65, 0.82, 1.0, 0.42)
    };

    parent
        .spawn((
            Button,
            action,
            ButtonVisualStyle {
                normal: if icon_handle.is_some() {
                    Color::WHITE
                } else {
                    base_color
                },
                hovered: Color::srgb(0.88, 0.96, 1.0),
                pressed: Color::srgb(0.68, 0.78, 0.92),
            },
            Node {
                width: Val::Px(104.0),
                height: Val::Px(74.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Stretch,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(if is_highlighted { 2.0 } else { 1.0 })),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(if icon_handle.is_some() {
                Color::WHITE
            } else {
                base_color
            }),
            BorderColor(border_col),
        ))
        .with_children(|btn| {
            // Full-bleed RA2 Cameo image background so the original baked-in unit/building name at the bottom is crisp
            if let Some(img) = icon_handle.clone() {
                btn.spawn((
                    ImageNode::new(img),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
            }

            // Top row: optional active status badge on left + compact $Cost pill on right
            btn.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .with_children(|top_row| {
                if !status_badge.is_empty() {
                    top_row
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.42, 0.16, 0.88)),
                        ))
                        .with_children(|badge| {
                            badge.spawn((
                                Text::new(sanitize_ui_text(status_badge)),
                                TextFont {
                                    font_size: 8.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.75, 1.0, 0.78)),
                            ));
                        });
                } else {
                    top_row.spawn(Node::default());
                }

                top_row
                    .spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(3.5), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
                    ))
                    .with_children(|cost_pill| {
                        cost_pill.spawn((
                            Text::new(sanitize_ui_text(cost_label)),
                            TextFont {
                                font_size: 9.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.90, 0.38)),
                        ));
                    });
            });

            // Only show fallback text label if the mod entity has no cameo image
            if icon_handle.is_none() {
                btn.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(3.0), Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
                ))
                .with_children(|lbl_box| {
                    lbl_box.spawn((
                        Text::new(sanitize_ui_text(short_title)),
                        TextFont {
                            font_size: 9.5,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
}

#[allow(clippy::type_complexity)]
pub fn update_selection_drag_box_ui(
    interaction: Res<ClientInteractionState>,
    mut drag_box_q: Query<
        &mut Node,
        (
            With<SelectionDragBoxNode>,
            Without<StudioModalRoot>,
            Without<ProductionProgressBarFill>,
            Without<BattleFeedRoot>,
            Without<InGameHudNode>,
            Without<TopBarRootNode>,
            Without<DevToolbarNode>,
            Without<RightSidebarRootNode>,
        ),
    >,
) {
    if let Ok(mut box_node) = drag_box_q.get_single_mut() {
        if !interaction.studio_open {
            if let (Some(a), Some(b)) =
                (interaction.drag_start_screen, interaction.drag_current_screen)
            {
                if a.distance(b) > 8.0 {
                    let min_p = a.min(b);
                    let size = (a - b).abs();
                    box_node.display = Display::Flex;
                    box_node.left = Val::Px(min_p.x);
                    box_node.top = Val::Px(min_p.y);
                    box_node.width = Val::Px(size.x);
                    box_node.height = Val::Px(size.y);
                    return;
                }
            }
        }
        box_node.display = Display::None;
    }
}

type ButtonHoverQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static ButtonVisualStyle,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

pub fn update_button_hover_styles(mut q: ButtonHoverQuery) {
    for (interaction, style, mut bg, mut border) in &mut q {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(style.pressed);
                *border = BorderColor(Color::srgb(0.98, 0.85, 0.35));
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(style.hovered);
                *border = BorderColor(Color::srgb(0.75, 0.90, 1.0));
            }
            Interaction::None => {
                *bg = BackgroundColor(style.normal);
                *border = BorderColor(Color::srgba(0.85, 0.92, 1.0, 0.25));
            }
        }
    }
}
