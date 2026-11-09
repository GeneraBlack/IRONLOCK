use crate::input::{MenuScreen, ProductionSidebarTab};
use rts_simulation::ai::AiDifficulty;
mod actions;
mod actions_studio;
mod hud_update;
mod inspector;
mod menu_screens;
mod minimap;
mod sidebar;
mod studio_layout;

pub use actions::handle_sidebar_buttons;
pub use hud_update::update_rts_hud;
pub use minimap::update_and_interact_radar_minimap;
pub use studio_layout::{
    spawn_icon_styled_button, spawn_styled_button, update_button_hover_styles,
    update_selection_drag_box_ui,
};
use studio_layout::spawn_studio_modal;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::RelativeCursorPosition;
use std::path::PathBuf;

#[derive(Resource, Clone)]
pub struct RadarMinimapImage(pub Handle<Image>);

#[derive(Component)]
pub struct RadarMinimapNode;

#[derive(Component)]
pub struct ProductionProgressBarFill;

#[derive(Component)]
pub struct BattleFeedRoot;

#[derive(Component)]
pub struct BattleFeedText;

#[derive(Component)]
pub struct TopBarStatusText;

#[derive(Component)]
pub struct SidebarQueueText;

#[derive(Component)]
pub struct BottomInspectorText;

#[derive(Component)]
pub struct DynamicButtonsContainer;

#[derive(Component)]
pub struct StudioModalRoot;

#[derive(Component)]
pub struct StudioStatusText;

#[derive(Component)]
pub struct StudioModListContainer;

#[derive(Component)]
pub struct SelectionDragBoxNode;

#[derive(Component)]
pub struct InGameHudNode;

#[derive(Component)]
pub struct TopBarRootNode;

#[derive(Component)]
pub struct DevToolbarNode;

#[derive(Component)]
pub struct RightSidebarRootNode;

#[derive(Component)]
pub struct BottomCommandBarRootNode;

#[derive(Component)]
pub struct PlayerBottomStancePillNode;

#[derive(Component)]
pub struct PlayerBottomStanceText;

#[derive(Component)]
pub struct SidebarCreditsText;

#[derive(Component)]
pub struct SidebarCreditsBgNode;

#[derive(Component)]
pub struct SidebarTopBgNode;

#[derive(Component)]
pub struct SidebarOptBtnNode;

#[derive(Component)]
pub struct SidebarDiploBtnNode;

#[derive(Component)]
pub struct SidebarRadarFrameBgNode;

#[derive(Component)]
pub struct SidebarTelemetryStripNode;

#[derive(Component)]
pub struct StudioTopDeckNode;

#[derive(Component)]
pub struct StudioHeaderTitleText;

#[derive(Component)]
pub struct StudioTab6ButtonText;

#[derive(Component, Clone)]
pub struct ButtonVisualStyle {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
}

#[derive(Component, Clone)]
pub enum SidebarAction {
    StartPlaceBuilding(String),
    ProduceUnit(String),
    ResearchUpgrade(String),
    TriggerAbility,
    ToggleAttackMove,
    CycleSelectedStance,
    TogglePatrolMode,
    ToggleGuardMode,
    StopSelectedUnits,
    ScatterSelectedUnits,
    CenterCameraOnSelected,
    CenterCameraOnBase,
    SelectControlGroup(usize),
    SwitchFaction,
    SelectPlayerFaction(u8),
    ToggleSkirmishAi,
    CycleAiDifficulty,
    SetAiDifficulty(AiDifficulty),
    ToggleFogOfWar,
    CycleGameSpeed,
    TogglePause,
    RepairSelectedBuildings,
    SellSelectedBuildings,
    UpgradeSelectedBuilding,
    SetPrimaryBuilding,
    UnloadSelectedCarriers,
    CycleAiPersonality,
    CycleGameMode,
    CycleEnvironmentPreset,
    QuickSaveMatch,
    QuickLoadMatch,
    CycleMapEditorBrush,
    ExportEditedMap,
    SwitchActiveMap(String),
    ToggleScoreboard,
    StartReplayPlayback,
    ToggleStudio,
    RestartMatch,
    LoadModFromPath(PathBuf),
    ValidateActiveMod,
    RegenerateActiveModGlb,
    ConvertDemoPacks,
    AutoScanLegacyImportsDir,
    HostLocalMultiplayer,
    JoinLocalMultiplayer,
    CycleHostPlayerSlots,
    BroadcastWorldSync,
    LoadSaveFile(PathBuf),
    PlayReplayFile(PathBuf),
    PlaceLandmine,
    StartCampaignMission {
        campaign_id: String,
        mission_id: String,
    },
    AdvanceNextCampaignMission,
    ToggleAllianceWithPlayer(u8),
    TransferCreditsToPlayer(u8, i32),
    TransferSelectedUnitsToPlayer(u8),
    ToggleIpInputEditing,
    SeekReplayRelative(i64),
    CycleReplayObserverPov,
    FilterSelectionToUnitType(String),
    CycleMasterVolume,
    CycleCameraPanSpeed,
    TriggerSupportPower(String),
    ToggleTechTree,
    RefreshOnlineLobbies,
    JoinOnlineLobby(String),
    NavigateMenu(MenuScreen),
    EnterLoadingScreen(String),
    FinishLoadingEnterGame,
    ReturnToMainMenu,
    SelectCampaignTab(String),
    CycleSkirmishCountry,
    SelectEncyclopediaFaction(String),
    InspectEncyclopediaEntry(String),
    ToggleLobbyServerEndpoint,
    HotReloadMod,
    CycleEditorBrushSize,
    CycleEditorSymmetry,
    ValidateMapPlayability,
    RunBalanceBenchmark,
    SelectCampaignById(String),
    CycleCampaignMissionPage(i32),
    CycleSkirmishMapPage(i32),
    ToggleSkirmishMapFilter,
    GenerateProceduralSkirmishMap,
    PlayModVideo(String),
    PlayModMusicTrack(String),
    ToggleDevMode,
    OpenInGamePauseMenu,
    SelectProductionTab(ProductionSidebarTab),
}

#[derive(Resource, Default)]
pub struct LastSidebarSignature {
    pub sidebar_sig: String,
    pub studio_sig: String,
}

pub fn setup_rts_hud(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let radar_size = 144u32;
    let radar_pixels = vec![24u8; (radar_size * radar_size * 4) as usize];
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

    // 2. Top Tactical Command Bar (Row 1: Clean Player HUD Bar; Row 2: Optional Developer Sandbox Toolbar)
    commands
        .spawn((
            InGameHudNode,
            TopBarRootNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(38.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Stretch,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.96)),
            BorderColor(Color::srgb(0.24, 0.36, 0.54)),
        ))
        .with_children(|top| {
            // Row 1: Player Game Mode Status & Match Controls
            top.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|player_row| {
                player_row.spawn((
                    TopBarStatusText,
                    Text::new("COMMAND & CONQUER: RED ALERT 2"),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.90, 0.65)),
                ));

                player_row
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(5.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|p_btns| {
                        spawn_styled_button(
                            p_btns,
                            "MENU [ESC]",
                            SidebarAction::OpenInGamePauseMenu,
                            Color::srgb(0.18, 0.36, 0.58),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "PAUSE [P]",
                            SidebarAction::TogglePause,
                            Color::srgb(0.42, 0.28, 0.16),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "SPEED [F6]",
                            SidebarAction::CycleGameSpeed,
                            Color::srgb(0.24, 0.34, 0.22),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "SAVE [F7]",
                            SidebarAction::QuickSaveMatch,
                            Color::srgb(0.18, 0.36, 0.28),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "LOAD [F8]",
                            SidebarAction::QuickLoadMatch,
                            Color::srgb(0.22, 0.34, 0.44),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "TECH [F9]",
                            SidebarAction::ToggleTechTree,
                            Color::srgb(0.20, 0.38, 0.46),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "DIPLOMACY [TAB]",
                            SidebarAction::ToggleScoreboard,
                            Color::srgb(0.32, 0.36, 0.18),
                            Val::Auto,
                        );
                        spawn_styled_button(
                            p_btns,
                            "DEV [F12]",
                            SidebarAction::ToggleDevMode,
                            Color::srgb(0.28, 0.22, 0.38),
                            Val::Auto,
                        );
                    });
            });

            // Row 2: Engine / Developer Sandbox Toolbar (hidden in Player Game Mode)
            top.spawn((
                DevToolbarNode,
                Node {
                    width: Val::Percent(100.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(5.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|toolbar| {
                spawn_styled_button(
                    toolbar,
                    "DEV-STUDIO",
                    SidebarAction::ToggleStudio,
                    Color::srgb(0.18, 0.38, 0.62),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "HOT-RELOAD",
                    SidebarAction::HotReloadMod,
                    Color::srgb(0.18, 0.44, 0.32),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "RESTART",
                    SidebarAction::RestartMatch,
                    Color::srgb(0.36, 0.24, 0.18),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "FACTION [F1/F2]",
                    SidebarAction::SwitchFaction,
                    Color::srgb(0.22, 0.26, 0.34),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "AI ON/OFF",
                    SidebarAction::ToggleSkirmishAi,
                    Color::srgb(0.28, 0.20, 0.38),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "AI DIFF [F4]",
                    SidebarAction::CycleAiDifficulty,
                    Color::srgb(0.36, 0.20, 0.42),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "FOG CHEAT [F5]",
                    SidebarAction::ToggleFogOfWar,
                    Color::srgb(0.16, 0.34, 0.36),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "MAP BRUSH [F10]",
                    SidebarAction::CycleMapEditorBrush,
                    Color::srgb(0.38, 0.26, 0.44),
                    Val::Auto,
                );
                spawn_styled_button(
                    toolbar,
                    "EXPORT TOML [F11]",
                    SidebarAction::ExportEditedMap,
                    Color::srgb(0.26, 0.38, 0.30),
                    Val::Auto,
                );
            });
        });

    // 2b. Top-Left EVA Tactical Battle Feed Banner
    commands
        .spawn((
            BattleFeedRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(46.0),
                max_width: Val::Px(940.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.88)),
            BorderColor(Color::srgb(0.26, 0.48, 0.72)),
        ))
        .with_children(|feed| {
            feed.spawn((
                BattleFeedText,
                Text::new("EVA ONLINE"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.95, 0.72)),
            ));
        });

    // 3. Right Tactical Production & Construction Sidebar (Authentic RA2 268px Vertical Stack)
    commands
        .spawn((
            InGameHudNode,
            RightSidebarRootNode,
            Node {
                width: Val::Px(268.0),
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(0.0)),
                row_gap: Val::Px(0.0),
                border: UiRect::left(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.98)),
            BorderColor(Color::srgb(0.28, 0.42, 0.62)),
        ))
        .with_children(|sidebar| {
            // 3.1 RA2 Credits Digital Readout Header (`credits.png`)
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
                        SidebarCreditsText,
                        Text::new("$ 10000"),
                        TextFont {
                            font_size: 13.5,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.92, 0.36)),
                    ));
                });

            // 3.2 RA2 Top Control Arch (`top.png` housing `optbtn.png` & `diplobtn.png`)
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
                        SidebarAction::OpenInGamePauseMenu,
                        ImageNode::default(),
                        ButtonVisualStyle {
                            normal: Color::WHITE,
                            hovered: Color::srgb(0.85, 0.95, 1.0),
                            pressed: Color::srgb(0.65, 0.78, 0.95),
                        },
                        Node {
                            width: Val::Px(112.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                    ));

                    top_arch.spawn((
                        SidebarDiploBtnNode,
                        Button,
                        SidebarAction::ToggleScoreboard,
                        ImageNode::default(),
                        ButtonVisualStyle {
                            normal: Color::WHITE,
                            hovered: Color::srgb(0.85, 0.95, 1.0),
                            pressed: Color::srgb(0.65, 0.78, 0.95),
                        },
                        Node {
                            width: Val::Px(112.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                    ));
                });

            // 3.3 RA2 Faction Emblem Radar Frame (`radar.png` / `radary.png` + interactive `RadarMinimapNode`)
            sidebar
                .spawn((
                    SidebarRadarFrameBgNode,
                    ImageNode::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(162.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::axes(Val::Px(0.0), Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(Color::srgb(0.28, 0.44, 0.66)),
                ))
                .with_children(|radar_frame| {
                    radar_frame.spawn((
                        RadarMinimapNode,
                        Button,
                        RelativeCursorPosition::default(),
                        ImageNode::new(radar_handle),
                        Node {
                            width: Val::Px(192.0),
                            height: Val::Px(138.0),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BorderColor(Color::srgba(0.38, 0.78, 1.0, 0.75)),
                    ));
                });

            // 3.4 Compact Telemetry & Build Progress Strip (shown in Dev Mode; hidden in Player Mode so `side1.png` connects directly to `radar.png`)
            sidebar
                .spawn((
                    InGameHudNode,
                    SidebarTelemetryStripNode,
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
                        SidebarQueueText,
                        Text::new("Status: Ready"),
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
                                ProductionProgressBarFill,
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.22, 0.92, 0.46)),
                            ));
                        });
                });

            // 3.5 Dynamic RA2 `side1.png` (Repair/Sell + 4 Icon Tabs), `side2.png` (Power Bar + 2-Col Full Cameos) & `side3.png`
            sidebar.spawn((
                DynamicButtonsContainer,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(0.0),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
            ));
        });

    // 3b. Compact Bottom-Left RA2 Tactical Selection & Stance Strip (Player Game Mode)
    commands
        .spawn((
            PlayerBottomStancePillNode,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(8.0),
                display: Display::None,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.12, 0.84)),
            BorderColor(Color::srgba(0.32, 0.62, 0.92, 0.75)),
        ))
        .with_children(|pill| {
            pill.spawn((
                PlayerBottomStanceText,
                Text::new(""),
                TextFont {
                    font_size: 10.5,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.95, 1.0)),
            ));
            spawn_styled_button(
                pill,
                "DEPLOY [G]",
                SidebarAction::TriggerAbility,
                Color::srgb(0.42, 0.30, 0.12),
                Val::Auto,
            );
            spawn_styled_button(
                pill,
                "STANCE [T]",
                SidebarAction::CycleSelectedStance,
                Color::srgb(0.24, 0.28, 0.42),
                Val::Auto,
            );
            spawn_styled_button(
                pill,
                "ATK-MOVE [X]",
                SidebarAction::ToggleAttackMove,
                Color::srgb(0.44, 0.22, 0.14),
                Val::Auto,
            );
            spawn_styled_button(
                pill,
                "STOP [H]",
                SidebarAction::StopSelectedUnits,
                Color::srgb(0.36, 0.18, 0.20),
                Val::Auto,
            );
        });

    // 4. Bottom Selection Inspector & Tactical Command Bar (86px height — visible in Engine Developer Mode [F12])
    commands
        .spawn((
            InGameHudNode,
            BottomCommandBarRootNode,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(268.0),
                bottom: Val::Px(0.0),
                height: Val::Px(86.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                column_gap: Val::Px(14.0),
                border: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.09, 0.13, 0.95)),
            BorderColor(Color::srgb(0.22, 0.30, 0.44)),
        ))
        .with_children(|bottom| {
            bottom
                .spawn(Node {
                    max_width: Val::Px(365.0),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|insp_wrap| {
                    insp_wrap.spawn((
                        BottomInspectorText,
                        Text::new("No unit selected."),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.86, 0.91, 0.96)),
                    ));
                });

            // Right-side Tactical Order & Control Group Toolbar
            bottom
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    align_items: AlignItems::FlexEnd,
                    ..default()
                })
                .with_children(|orders_col| {
                    orders_col
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|row1| {
                            spawn_styled_button(
                                row1,
                                "STANCE [T]",
                                SidebarAction::CycleSelectedStance,
                                Color::srgb(0.32, 0.28, 0.44),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "PATROL [Z]",
                                SidebarAction::TogglePatrolMode,
                                Color::srgb(0.18, 0.36, 0.48),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "GUARD [F]",
                                SidebarAction::ToggleGuardMode,
                                Color::srgb(0.16, 0.40, 0.36),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "ATK [X]",
                                SidebarAction::ToggleAttackMove,
                                Color::srgb(0.48, 0.24, 0.14),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "STOP [H]",
                                SidebarAction::StopSelectedUnits,
                                Color::srgb(0.38, 0.18, 0.20),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "SCATTER [V]",
                                SidebarAction::ScatterSelectedUnits,
                                Color::srgb(0.26, 0.32, 0.18),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "ABIL [G]",
                                SidebarAction::TriggerAbility,
                                Color::srgb(0.44, 0.32, 0.12),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "UNIT [C]",
                                SidebarAction::CenterCameraOnSelected,
                                Color::srgb(0.18, 0.32, 0.46),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row1,
                                "BASE",
                                SidebarAction::CenterCameraOnBase,
                                Color::srgb(0.18, 0.28, 0.40),
                                Val::Auto,
                            );
                        });

                    orders_col
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(5.0),
                            ..default()
                        })
                        .with_children(|row2| {
                            spawn_styled_button(
                                row2,
                                "UNLOAD [U]",
                                SidebarAction::UnloadSelectedCarriers,
                                Color::srgb(0.16, 0.38, 0.46),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row2,
                                "REPAIR [R]",
                                SidebarAction::RepairSelectedBuildings,
                                Color::srgb(0.18, 0.42, 0.24),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row2,
                                "SELL [DEL]",
                                SidebarAction::SellSelectedBuildings,
                                Color::srgb(0.46, 0.26, 0.14),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row2,
                                "UPGRADE [Y]",
                                SidebarAction::UpgradeSelectedBuilding,
                                Color::srgb(0.48, 0.36, 0.12),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row2,
                                "PRIMARY [B]",
                                SidebarAction::SetPrimaryBuilding,
                                Color::srgb(0.42, 0.36, 0.14),
                                Val::Auto,
                            );
                            spawn_styled_button(
                                row2,
                                "MINE [M]",
                                SidebarAction::PlaceLandmine,
                                Color::srgb(0.46, 0.22, 0.18),
                                Val::Auto,
                            );
                            for g in 0..5 {
                                let lbl = format!("GRP {}", g + 1);
                                spawn_styled_button(
                                    row2,
                                    &lbl,
                                    SidebarAction::SelectControlGroup(g),
                                    Color::srgb(0.20, 0.24, 0.34),
                                    Val::Auto,
                                );
                            }
                        });
                });
        });

    spawn_studio_modal(&mut commands);
}
