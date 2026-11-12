use super::*;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui::RelativeCursorPosition;

use super::player_windows::PlayerSubWindowRoot;
use super::studio_layout::spawn_studio_modal;

pub fn setup_rts_hud(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut fonts: Option<ResMut<Assets<Font>>>,
    smoke: Option<Res<crate::VisualSmokeTestState>>,
) {
    let ui_font = UiFontResource::load_or_fallback(fonts.as_deref_mut());
    commands.insert_resource(ui_font);
    commands.init_resource::<RtsCursorState>();
    commands.init_resource::<PlayerSubWindowStack>();
    commands.init_resource::<UiInputBlocker>();

    if smoke.as_ref().is_some_and(|s| s.output_dir.is_some()) {
        let mut smoke_img = Image::new_fill(
            Extent3d {
                width: 1440,
                height: 900,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        smoke_img.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC;
        let smoke_egui_handle = images.add(smoke_img);

        commands.spawn((
            SmokeTestEguiTarget,
            bevy_egui::EguiRenderToImage::new(smoke_egui_handle.clone()),
        ));
        commands.spawn((
            HudLayoutRole::SmokeTestEguiOverlay,
            ImageNode::new(smoke_egui_handle),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            ZIndex(500),
            PickingBehavior::IGNORE,
        ));
    }

    let radar_size = 256u32;
    let radar_pixels: Vec<u8> = [10u8, 14, 20, 255]
        .iter()
        .copied()
        .cycle()
        .take((radar_size * radar_size * 4) as usize)
        .collect();
    let radar_img = Image::new(
        Extent3d {
            width: radar_size,
            height: radar_size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        radar_pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let radar_handle = images.add(radar_img);
    commands.insert_resource(RadarMinimapImage(radar_handle.clone()));

    // 1. 2D Selection Drag Box overlay
    commands.spawn((
        SelectionDragBoxNode,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(0.0),
            height: Val::Px(0.0),
            display: Display::None,
            border: UiRect::all(Val::Px(1.5)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.95, 0.35, 0.18)),
        BorderColor(Color::srgb(0.2, 0.95, 0.35)),
    ));

    // 2. Clean Player Top Tactical & Multi-Resource Bar (left of 268px Faction Sidebar; zero dev clutter!)
    commands
        .spawn((
            HudLayoutRole::TopBarRoot,
            HudPanelBlocker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(268.0),
                top: Val::Px(0.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)),
                border: UiRect::bottom(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.92)),
            BorderColor(Color::srgb(0.24, 0.38, 0.58)),
        ))
        .with_children(|top| {
            top.spawn((
                HudTextRole::TopBarStatus,
                Text::new("COMMAND ONLINE"),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.90, 0.62)),
            ));

            top.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|btns| {
                spawn_styled_button(
                    btns,
                    "IDLE [.]",
                    SidebarAction::CycleIdleWorker,
                    Color::srgb(0.20, 0.36, 0.28),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "* COMMANDER",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::CommanderTree),
                    Color::srgb(0.38, 0.30, 0.14),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "JUKEBOX [J]",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Jukebox),
                    Color::srgb(0.24, 0.22, 0.42),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "BRIEFING",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Briefing),
                    Color::srgb(0.24, 0.28, 0.42),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "LOG [TAB]",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Logbook),
                    Color::srgb(0.18, 0.32, 0.44),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "TECH [Y]",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::TechTree),
                    Color::srgb(0.18, 0.38, 0.34),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "DIPLOMACY [D]",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Diplomacy),
                    Color::srgb(0.34, 0.30, 0.16),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "OPTIONS",
                    SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Options),
                    Color::srgb(0.22, 0.26, 0.36),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    "MENU [ESC]",
                    SidebarAction::OpenInGamePauseMenu,
                    Color::srgb(0.40, 0.18, 0.20),
                    Val::Auto,
                );
            });
        });

    // 2b. Top-Left EVA Tactical Battle Feed Banner
    commands
        .spawn((
            HudLayoutRole::BattleFeedRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(40.0),
                max_width: Val::Px(820.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.86)),
            BorderColor(Color::srgb(0.26, 0.48, 0.72)),
        ))
        .with_children(|feed| {
            feed.spawn((
                HudTextRole::BattleFeed,
                Text::new("EVA ONLINE"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.95, 0.72)),
            ));
        });

    // 3. Right Tactical Production & Construction Sidebar (268px Classic / 4x3 Bottom Card / Compact Dock)
    commands
        .spawn((
            HudLayoutRole::RightSidebarRoot,
            HudPanelBlocker,
            Node {
                width: Val::Px(268.0),
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::left(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.98)),
            BorderColor(Color::srgb(0.28, 0.42, 0.62)),
        ))
        .with_children(|sidebar| {
            sidebar
                .spawn((
                    SidebarCreditsBgNode,
                    ImageNode::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(26.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::bottom(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(Color::srgb(0.32, 0.48, 0.68)),
                ))
                .with_children(|cred| {
                    cred.spawn((
                        HudTextRole::SidebarCredits,
                        Text::new("$ 10000"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.92, 0.36)),
                    ));
                });

            sidebar
                .spawn((
                    SidebarTopBgNode,
                    ImageNode::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(42.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceEvenly,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        ..default()
                    },
                ))
                .with_children(|top_arch| {
                    top_arch.spawn((
                        SidebarOptBtnNode,
                        Button,
                        SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Options),
                        ImageNode::default(),
                        ButtonVisualStyle {
                            normal: Color::WHITE,
                            hovered: Color::srgb(0.85, 0.95, 1.0),
                            pressed: Color::srgb(0.65, 0.78, 0.95),
                            default_border: Color::NONE,
                        },
                        Node {
                            width: Val::Px(112.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        BorderColor(Color::NONE),
                    ));

                    top_arch.spawn((
                        SidebarDiploBtnNode,
                        Button,
                        SidebarAction::TogglePlayerWindow(PlayerSubWindowKind::Diplomacy),
                        ImageNode::default(),
                        ButtonVisualStyle {
                            normal: Color::WHITE,
                            hovered: Color::srgb(0.85, 0.95, 1.0),
                            pressed: Color::srgb(0.65, 0.78, 0.95),
                            default_border: Color::NONE,
                        },
                        Node {
                            width: Val::Px(112.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        BorderColor(Color::NONE),
                    ));
                });

            // Spacer inside ClassicRightSidebar reserving the 162px Radar slot
            sidebar.spawn((
                HudLayoutRole::ClassicRadarSlotSpacer,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(162.0),
                    ..default()
                },
            ));

            sidebar
                .spawn((
                    HudLayoutRole::SidebarTelemetryStrip,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.96)),
                ))
                .with_children(|status_box| {
                    status_box.spawn((
                        HudTextRole::SidebarQueue,
                        Text::new("Production Ready"),
                        TextFont {
                            font_size: 9.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.90, 1.0)),
                    ));
                    status_box
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(5.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.10, 0.14, 0.20)),
                            BorderColor(Color::srgb(0.24, 0.36, 0.52)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                HudLayoutRole::ProductionProgressFill,
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.22, 0.92, 0.46)),
                            ));
                        });
                });

            sidebar.spawn((
                DynamicButtonsContainer,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip_y(),
                    ..default()
                },
            ));
        });

    // 3b. Floating/Docked Radar Frame + Live Radar Minimap + Radar PIP FMV Overlay (all 3 HudLayoutPresets)
    commands
        .spawn((
            HudLayoutRole::RadarPanelRoot,
            HudPanelBlocker,
            SidebarRadarFrameBgNode,
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(68.0),
                width: Val::Px(268.0),
                height: Val::Px(162.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::axes(Val::Px(0.0), Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.98)),
            BorderColor(Color::srgb(0.28, 0.44, 0.66)),
            ZIndex(26),
        ))
        .with_children(|radar_frame| {
            radar_frame
                .spawn((
                    super::hud_layouts::RadarInnerWellNode,
                    Node {
                        width: Val::Px(192.0),
                        height: Val::Px(138.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.5)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.05, 0.08)),
                    BorderColor(Color::srgba(0.38, 0.78, 1.0, 0.75)),
                ))
                .with_children(|screen_well| {
                    screen_well
                        .spawn((
                            RadarMinimapNode,
                            Button,
                            RelativeCursorPosition::default(),
                            ImageNode::new(radar_handle),
                            Node {
                                width: Val::Px(192.0),
                                height: Val::Px(138.0),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ))
                        .with_children(|pip_parent| {
                            pip_parent.spawn((
                                RadarPipVideoNode,
                                ImageNode::default(),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(0.0),
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    display: Display::None,
                                    ..default()
                                },
                            ));
                        });
                });
        });

    // 4. Compact Bottom-Left Tactical Selection, Squad/Builder & Ability Pill
    commands
        .spawn((
            HudLayoutRole::PlayerBottomStancePill,
            HudPanelBlocker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(8.0),
                max_width: Val::Px(960.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.12, 0.90)),
            BorderColor(Color::srgba(0.32, 0.62, 0.92, 0.80)),
        ))
        .with_children(|pill| {
            pill.spawn((
                HudTextRole::PlayerBottomStance,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.90, 0.96, 1.0)),
            ));
            pill.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|cmd_row| {
                cmd_row.spawn((
                    PlayerAbilityButtonsContainer,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        ..default()
                    },
                ));
                spawn_styled_button(
                    cmd_row,
                    "STANCE [T]",
                    SidebarAction::CycleSelectedStance,
                    Color::srgb(0.24, 0.28, 0.42),
                    Val::Auto,
                );
                spawn_styled_button(
                    cmd_row,
                    "ATK-MOVE [X]",
                    SidebarAction::ToggleAttackMove,
                    Color::srgb(0.44, 0.22, 0.14),
                    Val::Auto,
                );
                spawn_styled_button(
                    cmd_row,
                    "STOP [H]",
                    SidebarAction::StopSelectedUnits,
                    Color::srgb(0.36, 0.18, 0.20),
                    Val::Auto,
                );
                spawn_styled_button(
                    cmd_row,
                    "UNLOAD [U]",
                    SidebarAction::UnloadSelectedCarriers,
                    Color::srgb(0.18, 0.36, 0.44),
                    Val::Auto,
                );
            });
        });

    // 5. Floating Mod-Skinned In-Game Player Sub-Windows Root (`PlayerSubWindowStack`)
    commands.spawn((
        PlayerSubWindowRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        PickingBehavior::IGNORE,
    ));

    super::tactical_command_bar::spawn_phase21_24_overlays(&mut commands);
    spawn_studio_modal(&mut commands);
}
