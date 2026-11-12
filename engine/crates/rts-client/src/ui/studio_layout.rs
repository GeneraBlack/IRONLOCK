use super::*;
use crate::input::{ClientInteractionState, MenuScreen};
use bevy::prelude::*;

/// Holds the loaded full-UTF-8 system TrueType font handle (with automatic fallback to Bevy's default font).
#[derive(Resource, Debug, Clone, Default)]
pub struct UiFontResource {
    pub font: Handle<Font>,
    pub loaded_system_ttf: bool,
    pub source_path: String,
}

impl UiFontResource {
    pub const CANDIDATE_PATHS: &'static [&'static str] = &[
        "C:/Windows/Fonts/segoeui.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    pub fn load_or_fallback(fonts: Option<&mut Assets<Font>>) -> Self {
        if let Some(font_assets) = fonts {
            for candidate in Self::CANDIDATE_PATHS {
                if let Ok(bytes) = std::fs::read(candidate) {
                    if let Ok(font) = Font::try_from_bytes(bytes) {
                        let handle = font_assets.add(font);
                        return Self {
                            font: handle,
                            loaded_system_ttf: true,
                            source_path: (*candidate).to_string(),
                        };
                    }
                }
            }
        }
        Self {
            font: Handle::default(),
            loaded_system_ttf: false,
            source_path: "builtin-default".to_string(),
        }
    }
}

/// Repairs UTF-8 mojibake sequences while preserving 100% of native UTF-8 characters (`ä, ö, ü, ß`,
/// accented Latin, em-dashes, tactical glyphs, and Unicode symbols).
pub fn sanitize_ui_text(input: &str) -> String {
    let repaired = input
        .replace("â€”", "—")
        .replace("â€“", "–")
        .replace("â˜…", "★")
        .replace("âœ”", "✔")
        .replace("ðŸ”’", "[LOCK]")
        .replace("â€¢", "•")
        .replace("ðŸ’¬", "[CHAT]");

    let mut out = String::with_capacity(repaired.len());
    for ch in repaired.chars() {
        if ch == '\n' || ch == '\t' || !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

/// Automatically injects the full UTF-8 TrueType `Handle<Font>` into newly spawned UI text nodes
/// and keeps Bevy's global `UiScale` synchronized with `ClientInteractionState.ui_scale` (`0.75..=1.50`).
pub fn apply_utf8_font_and_ui_scale(
    interaction: Res<ClientInteractionState>,
    ui_font: Option<Res<UiFontResource>>,
    mut ui_scale: Option<ResMut<UiScale>>,
    mut added_text_q: Query<&mut TextFont, Added<TextFont>>,
) {
    let target_scale = interaction.ui_scale.clamp(0.75, 1.50);
    if let Some(scale_res) = ui_scale.as_deref_mut() {
        if (scale_res.0 - target_scale).abs() > 1e-4 {
            scale_res.0 = target_scale;
        }
    }

    if let Some(font_res) = ui_font.as_deref() {
        if font_res.loaded_system_ttf {
            for mut tf in &mut added_text_q {
                if tf.font == Handle::default() {
                    tf.font = font_res.font.clone();
                }
            }
        }
    }
}

pub(super) fn spawn_studio_modal(commands: &mut Commands) {
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
            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.16)),
        ))
        .with_children(|modal| {
            // Top Navigation Header Bar for Secondary Player Submenus (hidden on Root Main Menu, LoadingScreen & PauseMenu)
            modal
                .spawn((
                    StudioTopDeckNode,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(10.0)),
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.07, 0.12, 0.92)),
                    BorderColor(Color::srgb(0.32, 0.58, 0.92)),
                ))
                .with_children(|top_deck| {
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
                                Text::new("COMMAND CENTER"),
                                TextFont {
                                    font_size: 14.0,
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
                                    "BACK TO MAIN MENU",
                                    SidebarAction::NavigateMenu(MenuScreen::Root),
                                    Color::srgb(0.18, 0.28, 0.44),
                                    Val::Auto,
                                );
                                spawn_styled_button(
                                    actions,
                                    "EXIT",
                                    SidebarAction::ExitApplication,
                                    Color::srgb(0.48, 0.14, 0.16),
                                    Val::Auto,
                                );
                            });
                        });

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
                                "MAIN MENU",
                                SidebarAction::NavigateMenu(MenuScreen::Root),
                                Color::srgb(0.16, 0.28, 0.46),
                                Val::Percent(14.0),
                            );
                            spawn_styled_button(
                                nav,
                                "CAMPAIGN",
                                SidebarAction::NavigateMenu(MenuScreen::Campaign),
                                Color::srgb(0.40, 0.26, 0.14),
                                Val::Percent(15.0),
                            );
                            spawn_styled_button(
                                nav,
                                "SKIRMISH",
                                SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                                Color::srgb(0.18, 0.38, 0.26),
                                Val::Percent(15.0),
                            );
                            spawn_styled_button(
                                nav,
                                "MULTIPLAYER",
                                SidebarAction::NavigateMenu(MenuScreen::Multiplayer),
                                Color::srgb(0.16, 0.38, 0.48),
                                Val::Percent(15.0),
                            );
                            spawn_styled_button(
                                nav,
                                "THEATER & REPLAYS",
                                SidebarAction::NavigateMenu(MenuScreen::Theater),
                                Color::srgb(0.32, 0.24, 0.44),
                                Val::Percent(15.0),
                            );
                            spawn_styled_button(
                                nav,
                                "ENCYCLOPEDIA",
                                SidebarAction::NavigateMenu(MenuScreen::Encyclopedia),
                                Color::srgb(0.24, 0.30, 0.48),
                                Val::Percent(14.0),
                            );
                            spawn_styled_button(
                                nav,
                                "OPTIONS",
                                SidebarAction::NavigateMenu(MenuScreen::Settings),
                                Color::srgb(0.24, 0.28, 0.36),
                                Val::Percent(12.0),
                            );
                        });

                    top_deck
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
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
    let default_border = Color::srgba(0.85, 0.92, 1.0, 0.28);

    parent
        .spawn((
            Button,
            action,
            ButtonVisualStyle {
                normal: base_color,
                hovered,
                pressed,
                default_border,
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
            BorderColor(default_border),
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

pub use super::sidebar_cameos::spawn_cameo_grid_button;

#[allow(clippy::type_complexity)]
pub fn update_selection_drag_box_ui(
    interaction: Res<ClientInteractionState>,
    mut drag_box_q: Query<
        &mut Node,
        (
            With<SelectionDragBoxNode>,
            Without<HudLayoutRole>,
            Without<StudioModalRoot>,
        ),
    >,
) {
    if let Ok(mut box_node) = drag_box_q.get_single_mut() {
        if !interaction.menu_open
            && !interaction.studio_open
            && interaction.app_phase == crate::input::AppPhase::InGame
        {
            if let (Some(a), Some(b)) = (
                interaction.drag_start_screen,
                interaction.drag_current_screen,
            ) {
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
                *border = BorderColor(style.default_border);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_ui_text_preserves_full_utf8_umlauts_and_repairs_mojibake() {
        let out = sanitize_ui_text("Panzerjäger Größe — Café Élite äöüß â˜… ðŸ”’");
        assert_eq!(out, "Panzerjäger Größe — Café Élite äöüß ★ [LOCK]");
    }
}
