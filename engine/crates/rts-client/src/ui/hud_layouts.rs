use crate::input::{AppPhase, ClientInteractionState, UiInputBlocker};
use crate::ui::control_groups_bar::ControlGroupsBarRoot;
use crate::ui::player_windows::PlayerSubWindowPanel;
use crate::ui::widgets::ui_node_logical_rect;
use crate::ui::{
    HudLayoutRole, HudPanelBlocker, RadarMinimapNode, SidebarCreditsBgNode, SidebarTopBgNode,
};
use crate::ModResource;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::manifest::HudLayoutPreset;

/// Marker on the inner dark bezel wrapping `RadarMinimapNode` inside `RadarPanelRoot`.
#[derive(Component)]
pub struct RadarInnerWellNode;

/// Marker on decorative RA2 right-sidebar chrome (`side3.png` / `addon.png`) hidden in compact/bottom presets.
#[derive(Component)]
pub struct ClassicSidebarChromeOnly;

/// Synchronizes all HUD root containers to the active [`HudLayoutPreset`] (`ClassicRightSidebar`,
/// `ModernBottomCommandCard`, `CompactBarOverlay`) AND collects live Bevy [`ComputedNode`] logical
/// screen rectangles into [`UiInputBlocker::computed_hud_rects`] (`Task 240`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_hud_layout_preset_and_computed_rects(
    interaction: Res<ClientInteractionState>,
    mod_res: Res<ModResource>,
    mut ui_blocker: ResMut<UiInputBlocker>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut layout_q: Query<(&mut Node, &HudLayoutRole), Without<ControlGroupsBarRoot>>,
    mut radar_well_q: Query<
        &mut Node,
        (
            With<RadarInnerWellNode>,
            Without<HudLayoutRole>,
            Without<RadarMinimapNode>,
            Without<ControlGroupsBarRoot>,
        ),
    >,
    mut radar_img_q: Query<
        &mut Node,
        (
            With<RadarMinimapNode>,
            Without<HudLayoutRole>,
            Without<RadarInnerWellNode>,
            Without<ControlGroupsBarRoot>,
        ),
    >,
    mut classic_chrome_q: Query<
        &mut Node,
        (
            Or<(
                With<SidebarCreditsBgNode>,
                With<SidebarTopBgNode>,
                With<ClassicSidebarChromeOnly>,
            )>,
            Without<HudLayoutRole>,
            Without<RadarInnerWellNode>,
            Without<RadarMinimapNode>,
            Without<ControlGroupsBarRoot>,
        ),
    >,
    mut ctrl_grp_q: Query<&mut Node, With<ControlGroupsBarRoot>>,
    blocker_q: Query<
        (&ComputedNode, &GlobalTransform, &InheritedVisibility),
        With<HudPanelBlocker>,
    >,
    subwin_q: Query<
        (&ComputedNode, &GlobalTransform, &InheritedVisibility),
        (With<PlayerSubWindowPanel>, Without<HudPanelBlocker>),
    >,
) {
    let show_ingame_hud = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;
    let preset = interaction.effective_hud_layout(&mod_res.0);

    for (mut node, role) in &mut layout_q {
        match role {
            HudLayoutRole::TopBarRoot => match preset {
                HudLayoutPreset::ClassicRightSidebar => {
                    node.left = Val::Px(0.0);
                    node.right = Val::Px(268.0);
                    node.top = Val::Px(0.0);
                    node.height = Val::Px(34.0);
                }
                HudLayoutPreset::ModernBottomCommandCard => {
                    node.left = Val::Px(0.0);
                    node.right = Val::Px(0.0);
                    node.top = Val::Px(0.0);
                    node.height = Val::Px(34.0);
                }
                HudLayoutPreset::CompactBarOverlay => {
                    node.left = Val::Px(0.0);
                    node.right = Val::Px(0.0);
                    node.top = Val::Px(0.0);
                    node.height = Val::Px(30.0);
                }
            },
            HudLayoutRole::RightSidebarRoot => match preset {
                HudLayoutPreset::ClassicRightSidebar => {
                    node.width = Val::Px(268.0);
                    node.right = Val::Px(0.0);
                    node.top = Val::Px(0.0);
                    node.bottom = Val::Px(0.0);
                    node.height = Val::Auto;
                    node.max_height = Val::Auto;
                    node.border = UiRect::left(Val::Px(2.0));
                }
                HudLayoutPreset::ModernBottomCommandCard => {
                    // Bottom-Right 4x3 Command/Build Card
                    node.width = Val::Px(404.0);
                    node.right = Val::Px(8.0);
                    node.top = Val::Auto;
                    node.bottom = Val::Px(8.0);
                    node.height = Val::Auto;
                    node.max_height = Val::Px(268.0);
                    node.border = UiRect::all(Val::Px(2.0));
                }
                HudLayoutPreset::CompactBarOverlay => {
                    // Minimalist SupCom/BAR compact floating dock
                    node.width = Val::Px(228.0);
                    node.right = Val::Px(6.0);
                    node.top = Val::Px(68.0);
                    node.bottom = Val::Auto;
                    node.height = Val::Auto;
                    node.max_height = Val::Px(420.0);
                    node.border = UiRect::all(Val::Px(1.5));
                }
            },
            HudLayoutRole::RadarPanelRoot => {
                node.display = if show_ingame_hud {
                    Display::Flex
                } else {
                    Display::None
                };
                match preset {
                    HudLayoutPreset::ClassicRightSidebar => {
                        node.right = Val::Px(0.0);
                        node.left = Val::Auto;
                        node.top = Val::Px(68.0);
                        node.bottom = Val::Auto;
                        node.width = Val::Px(268.0);
                        node.height = Val::Px(162.0);
                        node.border = UiRect::axes(Val::Px(0.0), Val::Px(1.0));
                    }
                    HudLayoutPreset::ModernBottomCommandCard => {
                        node.left = Val::Px(8.0);
                        node.right = Val::Auto;
                        node.top = Val::Auto;
                        node.bottom = Val::Px(8.0);
                        node.width = Val::Px(224.0);
                        node.height = Val::Px(176.0);
                        node.border = UiRect::all(Val::Px(2.0));
                    }
                    HudLayoutPreset::CompactBarOverlay => {
                        node.left = Val::Px(8.0);
                        node.right = Val::Auto;
                        node.top = Val::Auto;
                        node.bottom = Val::Px(8.0);
                        node.width = Val::Px(192.0);
                        node.height = Val::Px(146.0);
                        node.border = UiRect::all(Val::Px(1.5));
                    }
                }
            }
            HudLayoutRole::ClassicRadarSlotSpacer => {
                node.display = if show_ingame_hud && preset == HudLayoutPreset::ClassicRightSidebar
                {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::GlobalProductionStripRoot => match preset {
                HudLayoutPreset::ClassicRightSidebar => {
                    node.left = Val::Px(12.0);
                    node.right = Val::Px(278.0);
                    node.top = Val::Px(38.0);
                }
                HudLayoutPreset::ModernBottomCommandCard => {
                    node.left = Val::Px(12.0);
                    node.right = Val::Px(12.0);
                    node.top = Val::Px(38.0);
                }
                HudLayoutPreset::CompactBarOverlay => {
                    node.left = Val::Px(8.0);
                    node.right = Val::Px(8.0);
                    node.top = Val::Px(33.0);
                }
            },
            HudLayoutRole::HeroInventoryBarRoot => match preset {
                HudLayoutPreset::ClassicRightSidebar => {
                    node.left = Val::Px(10.0);
                    node.right = Val::Auto;
                    node.bottom = if interaction.selected_ids.is_empty() {
                        Val::Px(42.0)
                    } else {
                        Val::Px(112.0)
                    };
                    node.max_width = Val::Px(560.0);
                    node.max_height = Val::Auto;
                    node.overflow = Overflow::visible();
                }
                HudLayoutPreset::ModernBottomCommandCard => {
                    node.left = Val::Px(240.0);
                    node.right = Val::Px(420.0);
                    node.bottom = Val::Px(8.0);
                    node.max_width = Val::Auto;
                    node.max_height = Val::Px(128.0);
                    node.overflow = Overflow::clip();
                }
                HudLayoutPreset::CompactBarOverlay => {
                    node.left = Val::Px(208.0);
                    node.right = Val::Auto;
                    node.bottom = Val::Px(8.0);
                    node.max_width = Val::Px(520.0);
                    node.max_height = Val::Auto;
                    node.overflow = Overflow::visible();
                }
            },
            HudLayoutRole::PlayerBottomStancePill => match preset {
                HudLayoutPreset::ClassicRightSidebar => {
                    node.left = Val::Px(10.0);
                    node.right = Val::Auto;
                    node.bottom = Val::Px(8.0);
                    node.max_width = Val::Px(920.0);
                    node.max_height = Val::Auto;
                    node.overflow = Overflow::visible();
                }
                HudLayoutPreset::ModernBottomCommandCard => {
                    node.left = Val::Px(240.0);
                    node.right = Val::Px(420.0);
                    node.bottom = Val::Px(144.0);
                    node.max_width = Val::Px(780.0);
                    node.max_height = Val::Px(68.0);
                    node.overflow = Overflow::clip();
                }
                HudLayoutPreset::CompactBarOverlay => {
                    node.left = Val::Px(208.0);
                    node.right = Val::Auto;
                    node.bottom = Val::Px(118.0);
                    node.max_width = Val::Px(680.0);
                    node.max_height = Val::Auto;
                    node.overflow = Overflow::visible();
                }
            },
            HudLayoutRole::BattleFeedRoot => {
                node.top = match preset {
                    HudLayoutPreset::CompactBarOverlay => Val::Px(68.0),
                    _ => Val::Px(74.0),
                };
            }
            HudLayoutRole::CameoTooltipBanner => match preset {
                HudLayoutPreset::ClassicRightSidebar => {
                    node.right = Val::Px(276.0);
                    node.bottom = Val::Px(14.0);
                }
                HudLayoutPreset::ModernBottomCommandCard => {
                    node.right = Val::Px(12.0);
                    node.bottom = Val::Px(282.0);
                }
                HudLayoutPreset::CompactBarOverlay => {
                    node.right = Val::Px(240.0);
                    node.bottom = Val::Px(14.0);
                }
            },
            _ => {}
        }
    }

    let (well_w, well_h) = match preset {
        HudLayoutPreset::ClassicRightSidebar => (192.0, 138.0),
        HudLayoutPreset::ModernBottomCommandCard => (208.0, 160.0),
        HudLayoutPreset::CompactBarOverlay => (178.0, 132.0),
    };
    for mut wn in &mut radar_well_q {
        wn.width = Val::Px(well_w);
        wn.height = Val::Px(well_h);
    }
    for mut rn in &mut radar_img_q {
        rn.width = Val::Px(well_w);
        rn.height = Val::Px(well_h);
    }

    let show_classic_chrome = preset == HudLayoutPreset::ClassicRightSidebar;
    for mut cn in &mut classic_chrome_q {
        cn.display = if show_classic_chrome {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Ok(mut cg_node) = ctrl_grp_q.get_single_mut() {
        match preset {
            HudLayoutPreset::ClassicRightSidebar => {
                cg_node.left = Val::Px(10.0);
                cg_node.bottom = if interaction.selected_ids.is_empty() {
                    Val::Px(10.0)
                } else {
                    Val::Px(82.0)
                };
            }
            HudLayoutPreset::ModernBottomCommandCard => {
                cg_node.left = Val::Px(240.0);
                cg_node.bottom = Val::Px(218.0);
            }
            HudLayoutPreset::CompactBarOverlay => {
                cg_node.left = Val::Px(8.0);
                cg_node.bottom = Val::Px(160.0);
            }
        }
    }

    // Collect real Bevy `ComputedNode` logical HUD rectangles for accurate world-click blocking!
    ui_blocker.computed_hud_rects.clear();
    let Ok(window) = window_q.get_single() else {
        ui_blocker.over_player_hud = false;
        return;
    };
    if !show_ingame_hud {
        ui_blocker.over_player_hud = false;
        return;
    }

    let sf = window.scale_factor();
    for (computed, gt, vis) in blocker_q.iter().chain(subwin_q.iter()) {
        if !vis.get() {
            continue;
        }
        let rect = ui_node_logical_rect(computed, gt, sf);
        if rect.width() > 2.0 && rect.height() > 2.0 {
            ui_blocker.computed_hud_rects.push(rect);
        }
    }

    if let Some(cursor) = window.cursor_position() {
        ui_blocker.over_player_hud = ui_blocker.pointer_over_computed_hud(cursor);
    } else {
        ui_blocker.over_player_hud = false;
    }
}
