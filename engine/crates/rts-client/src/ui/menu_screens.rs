use super::*;
use crate::input::{
    mod_skirmish_option_count, resolve_mod_campaign_loading_screen, resolve_mod_entity_cameo,
    resolve_mod_skirmish_descriptor, AppPhase, ClientInteractionState, MenuScreen,
};
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::ai::AiDifficulty;
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub(super) fn populate_active_menu_screen(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    discovered_mods: &[PathBuf],
    save_files: &[PathBuf],
    replay_files: &[PathBuf],
    pid: u8,
) {
    if interaction.app_phase == AppPhase::LoadingScreen {
        spawn_loading_screen_view(parent, asset_server, loaded, interaction);
        return;
    }

    if !interaction.dev_mode_enabled && crate::input::mod_has_custom_hud_skin(loaded) {
        spawn_ra2_menu_shell(
            parent,
            asset_server,
            asset_root,
            loaded,
            interaction,
            discovered_mods,
            save_files,
            replay_files,
            pid,
        );
        return;
    }

    match interaction.active_menu_screen {
        MenuScreen::Root => spawn_root_main_menu(parent, asset_server, loaded, interaction),
        MenuScreen::Campaign => spawn_campaign_submenu(parent, asset_server, loaded, interaction),
        MenuScreen::Skirmish => {
            spawn_skirmish_submenu(parent, asset_server, loaded, interaction, pid)
        }
        MenuScreen::Multiplayer => spawn_multiplayer_submenu(parent, loaded, interaction),
        MenuScreen::Theater => {
            spawn_theater_submenu(parent, loaded, interaction, save_files, replay_files)
        }
        MenuScreen::Studio => spawn_studio_and_encyclopedia_submenu(
            parent,
            asset_server,
            asset_root,
            loaded,
            interaction,
            discovered_mods,
        ),
        MenuScreen::Settings => spawn_settings_submenu(parent, interaction),
        MenuScreen::PauseMenu => {
            spawn_pause_menu_submenu(parent, asset_server, loaded, interaction)
        }
    }
}

pub(super) fn load_ui_image(
    asset_server: Option<&AssetServer>,
    rel_path: &str,
) -> Option<Handle<Image>> {
    asset_server.map(|srv| {
        if !std::path::Path::new(rel_path).exists()
            && std::path::Path::new("rust-ra2").join(rel_path).exists()
        {
            srv.load(format!("rust-ra2/{rel_path}"))
        } else {
            srv.load(rel_path.to_string())
        }
    })
}

pub(super) fn load_mod_cameo(
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    entity_id: &str,
) -> Option<Handle<Image>> {
    resolve_mod_entity_cameo(loaded, entity_id)
        .and_then(|rel_path| load_ui_image(asset_server, &rel_path))
}

fn load_mod_showcase_cameo(
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    index: usize,
) -> Option<Handle<Image>> {
    let mut ids: Vec<&str> = loaded
        .units
        .keys()
        .map(|s| s.as_str())
        .chain(loaded.buildings.keys().map(|s| s.as_str()))
        .collect();
    if ids.is_empty() {
        return None;
    }
    ids.sort_unstable();
    let id = ids[index % ids.len()];
    load_mod_cameo(asset_server, loaded, id)
}

fn spawn_4k_artwork_banner(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    rel_path_opt: Option<&str>,
    height_px: f32,
) {
    if let Some(rel_path) = rel_path_opt {
        if let Some(handle) = load_ui_image(asset_server, rel_path) {
            parent.spawn((
                ImageNode::new(handle),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(height_px),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor(Color::srgb(0.95, 0.82, 0.38)),
            ));
        }
    }
}

fn spawn_stat_bar(
    parent: &mut ChildBuilder,
    label: &str,
    ratio: f32,
    fill_color: Color,
    value_text: &str,
) {
    let clamped_pct = (ratio.clamp(0.04, 1.0) * 100.0).round();
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|wrap| {
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new(studio_layout::sanitize_ui_text(label)),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.78, 0.86, 0.94)),
                ));
                hdr.spawn((
                    Text::new(studio_layout::sanitize_ui_text(value_text)),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.90, 0.55)),
                ));
            });

            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(9.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.95)),
                BorderColor(Color::srgb(0.28, 0.42, 0.62)),
            ))
            .with_children(|track| {
                track.spawn((
                    Node {
                        width: Val::Percent(clamped_pct),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                ));
            });
        });
}

fn spawn_column_card(
    parent: &mut ChildBuilder,
    width_pct: f32,
    title: &str,
    title_color: Color,
    build_children: impl FnOnce(&mut ChildBuilder),
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(width_pct),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(7.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.11, 0.03, 0.03, 0.84)),
            BorderColor(Color::srgb(0.84, 0.66, 0.24)),
        ))
        .with_children(|col| {
            col.spawn((
                Text::new(studio_layout::sanitize_ui_text(title)),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(title_color),
            ));
            build_children(col);
        });
}

fn spawn_ra2_sidebar_button(
    parent: &mut ChildBuilder,
    label: &str,
    action: SidebarAction,
    is_highlighted: bool,
) {
    let normal_bg = if is_highlighted {
        Color::srgb(0.68, 0.16, 0.12)
    } else {
        Color::srgb(0.46, 0.09, 0.09)
    };
    let hover_bg = if is_highlighted {
        Color::srgb(0.84, 0.24, 0.16)
    } else {
        Color::srgb(0.64, 0.14, 0.12)
    };
    let pressed_bg = Color::srgb(0.30, 0.05, 0.05);
    let border_col = if is_highlighted {
        Color::srgb(1.0, 0.88, 0.36)
    } else {
        Color::srgb(0.86, 0.68, 0.24)
    };

    parent
        .spawn((
            Button,
            action,
            ButtonVisualStyle {
                normal: normal_bg,
                hovered: hover_bg,
                pressed: pressed_bg,
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(42.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(normal_bg),
            BorderColor(border_col),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(studio_layout::sanitize_ui_text(label)),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.90, 0.36)),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
fn spawn_ra2_menu_shell(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    discovered_mods: &[PathBuf],
    save_files: &[PathBuf],
    replay_files: &[PathBuf],
    pid: u8,
) {
    let (country_name, skirmish_fac_id, _, country_load_img) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
    let active_skin_fac_id = if interaction.active_menu_screen == MenuScreen::Campaign {
        interaction
            .active_campaign_id
            .as_ref()
            .and_then(|cid| loaded.campaigns.get(cid))
            .map(|c| c.faction.clone())
            .unwrap_or_else(|| skirmish_fac_id.clone())
    } else {
        skirmish_fac_id.clone()
    };
    let hud_skin_opt =
        crate::input::resolve_active_faction_hud_skin(loaded, &active_skin_fac_id);

    let dlg_bg_path = hud_skin_opt
        .and_then(|s| s.dialog_background.as_deref())
        .map(|p| crate::input::qualify_mod_asset_rel_path(loaded, p));
    let radar_bg_path = hud_skin_opt
        .and_then(|s| s.radar_frame.as_deref())
        .map(|p| crate::input::qualify_mod_asset_rel_path(loaded, p));
    let addon_bg_path = hud_skin_opt
        .and_then(|s| s.addon_panel.as_deref())
        .map(|p| crate::input::qualify_mod_asset_rel_path(loaded, p));
    let observer_candidate =
        crate::input::qualify_mod_asset_rel_path(loaded, "assets/ui/loading_screens/observer.png");
    let observer_path = if std::path::Path::new(&observer_candidate).exists() {
        Some(observer_candidate)
    } else {
        country_load_img.clone()
    };

    let screen_title = match interaction.active_menu_screen {
        MenuScreen::Root => format!(
            "{} — COMMAND CENTER & MAIN MENU",
            loaded.manifest.name.to_uppercase()
        ),
        MenuScreen::Campaign => "SINGLEPLAYER — CAMPAIGN & MISSION BRIEFING".to_string(),
        MenuScreen::Skirmish => "SINGLEPLAYER — SKIRMISH CONFIGURATION".to_string(),
        MenuScreen::Multiplayer => "MULTIPLAYER — ONLINE LOBBY & LAN SKIRMISH".to_string(),
        MenuScreen::Theater => "SECRET ARCHIVES — REPLAY THEATER, VIDEOS & JUKEBOX".to_string(),
        MenuScreen::Studio => "INTELLIGENCE DOSSIER — 3D UNIT & STRUCTURE ENCYCLOPEDIA".to_string(),
        MenuScreen::Settings => "SYSTEM OPTIONS — GRAPHICS, AUDIO & HOTKEYS".to_string(),
        MenuScreen::PauseMenu => "BATTLE COMMAND — TACTICAL PAUSE MENU".to_string(),
    };

    // Full-Screen 2-Column Red Alert 2 Shell: Left/Center Main Viewport + Right 276px Command Column
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|shell| {
            // 1. Left/Center Main RA2 Screen Frame (`flex_grow: 1.0`, `height: 100%`)
            shell
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(if interaction.active_menu_screen == MenuScreen::Root {
                        Color::srgba(0.06, 0.01, 0.01, 0.18)
                    } else {
                        Color::srgba(0.08, 0.02, 0.02, 0.88)
                    }),
                    BorderColor(Color::srgb(0.76, 0.16, 0.14)),
                ))
                .with_children(|left_frame| {
                    // 1.1 Top RA2 Crimson/Gold Title Bar (`38px`)
                    left_frame
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(38.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                                border: UiRect::bottom(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.28, 0.05, 0.05)),
                            BorderColor(Color::srgb(0.88, 0.70, 0.24)),
                        ))
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new(studio_layout::sanitize_ui_text(&screen_title)),
                                TextFont {
                                    font_size: 14.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.88, 0.34)),
                            ));
                        });

                    // 1.2 Center Main Content Viewport (`flex_grow: 1.0`) backed by `bkgdlg.png` on submenus
                    left_frame
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Row,
                            padding: UiRect::all(Val::Px(12.0)),
                            column_gap: Val::Px(12.0),
                            ..default()
                        })
                        .with_children(|viewport| {
                            if interaction.active_menu_screen != MenuScreen::Root {
                                if let Some(ref bg_rel) = dlg_bg_path {
                                    if let Some(h) = load_ui_image(asset_server, bg_rel) {
                                        viewport.spawn((
                                            ImageNode::new(h)
                                                .with_color(Color::srgba(1.0, 1.0, 1.0, 0.42)),
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
                                }
                            }

                            match interaction.active_menu_screen {
                                MenuScreen::Root => {
                                    spawn_ra2_root_main_menu(
                                        viewport,
                                        asset_server,
                                        loaded,
                                        interaction,
                                    );
                                }
                                MenuScreen::Campaign => {
                                    spawn_campaign_submenu(
                                        viewport,
                                        asset_server,
                                        loaded,
                                        interaction,
                                    );
                                }
                                MenuScreen::Skirmish => {
                                    spawn_skirmish_submenu(
                                        viewport,
                                        asset_server,
                                        loaded,
                                        interaction,
                                        pid,
                                    );
                                }
                                MenuScreen::Multiplayer => {
                                    spawn_multiplayer_submenu(viewport, loaded, interaction);
                                }
                                MenuScreen::Theater => {
                                    spawn_theater_submenu(
                                        viewport,
                                        loaded,
                                        interaction,
                                        save_files,
                                        replay_files,
                                    );
                                }
                                MenuScreen::Studio => {
                                    spawn_studio_and_encyclopedia_submenu(
                                        viewport,
                                        asset_server,
                                        asset_root,
                                        loaded,
                                        interaction,
                                        discovered_mods,
                                    );
                                }
                                MenuScreen::Settings => {
                                    spawn_settings_submenu(viewport, interaction);
                                }
                                MenuScreen::PauseMenu => {
                                    spawn_pause_menu_submenu(
                                        viewport,
                                        asset_server,
                                        loaded,
                                        interaction,
                                    );
                                }
                            }
                        });

                    // 1.3 Bottom RA2 Status Bar (`32px`) with Status Readout & discrete `[DEV-STUDIO F12]` toggle
                    left_frame
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(32.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(3.0)),
                                border: UiRect::top(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.20, 0.04, 0.04)),
                            BorderColor(Color::srgb(0.84, 0.66, 0.24)),
                        ))
                        .with_children(|status_bar| {
                            status_bar.spawn((
                                StudioStatusText,
                                Text::new(studio_layout::sanitize_ui_text(
                                    &interaction.studio_status_message,
                                )),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.86, 0.36)),
                            ));

                            status_bar
                                .spawn(Node {
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                })
                                .with_children(|right_meta| {
                                    right_meta.spawn((
                                        Text::new(studio_layout::sanitize_ui_text(&format!(
                                            "{} v{}",
                                            loaded.manifest.id.to_uppercase(),
                                            loaded.manifest.version
                                        ))),
                                        TextFont {
                                            font_size: 10.5,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.84, 0.72, 0.48)),
                                    ));
                                    spawn_styled_button(
                                        right_meta,
                                        "ENGINE DEV MODE [F12]",
                                        SidebarAction::ToggleDevMode,
                                        Color::srgb(0.38, 0.14, 0.12),
                                        Val::Auto,
                                    );
                                });
                        });
                });

            // 2. Right-Hand Red Alert 2 Command Column (`276px` wide, `height: 100%`)
            shell
                .spawn((
                    Node {
                        width: Val::Px(276.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        border: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.04, 0.04)),
                    BorderColor(Color::srgb(0.86, 0.68, 0.24)),
                ))
                .with_children(|right_col| {
                    // 2.1 Top Faction Radar / Emblem Box (`164px` high)
                    let radar_h = radar_bg_path
                        .as_deref()
                        .and_then(|p| load_ui_image(asset_server, p));
                    let obs_h = observer_path
                        .as_deref()
                        .and_then(|p| load_ui_image(asset_server, p));
                    right_col
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(164.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::bottom(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(if radar_h.is_some() {
                                Color::WHITE
                            } else {
                                Color::srgb(0.24, 0.05, 0.05)
                            }),
                            BorderColor(Color::srgb(0.86, 0.68, 0.24)),
                            radar_h.map(ImageNode::new).unwrap_or_default(),
                        ))
                        .with_children(|emblem_box| {
                            if let Some(oh) = obs_h {
                                emblem_box.spawn((
                                    ImageNode::new(oh),
                                    Node {
                                        width: Val::Px(194.0),
                                        height: Val::Px(132.0),
                                        border: UiRect::all(Val::Px(1.5)),
                                        ..default()
                                    },
                                    BorderColor(Color::srgb(0.95, 0.80, 0.32)),
                                ));
                            }
                        });

                    // 2.2 Vertical Stack of Heavy Beveled Crimson/Gold RA2 Command Buttons
                    right_col
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::FlexStart,
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(12.0)),
                            row_gap: Val::Px(9.0),
                            ..default()
                        })
                        .with_children(|btns| {
                            btns.spawn((
                                Text::new("MAIN COMMAND"),
                                TextFont {
                                    font_size: 11.5,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.96, 0.82, 0.34)),
                            ));

                            match interaction.active_menu_screen {
                                MenuScreen::Root => {
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SINGLEPLAYER: CAMPAIGN",
                                        SidebarAction::NavigateMenu(MenuScreen::Campaign),
                                        true,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SINGLEPLAYER: SKIRMISH",
                                        SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "MULTIPLAYER (ONLINE/LAN)",
                                        SidebarAction::NavigateMenu(MenuScreen::Multiplayer),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "ARCHIVES & REPLAYS",
                                        SidebarAction::NavigateMenu(MenuScreen::Theater),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "UNIT ENCYCLOPEDIA",
                                        SidebarAction::NavigateMenu(MenuScreen::Studio),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "OPTIONS & AUDIO",
                                        SidebarAction::NavigateMenu(MenuScreen::Settings),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "★ QUICKSTART: SKIRMISH",
                                        SidebarAction::EnterLoadingScreen(format!(
                                            "SKIRMISH — {} ({})",
                                            loaded.entry_map.name, country_name
                                        )),
                                        true,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "▶ LIVE BATTLEFIELD [ESC]",
                                        SidebarAction::FinishLoadingEnterGame,
                                        false,
                                    );
                                }
                                MenuScreen::Campaign => {
                                    let sel_cid = interaction
                                        .active_campaign_id
                                        .clone()
                                        .or_else(|| loaded.campaigns.keys().next().cloned())
                                        .unwrap_or_else(|| "allied".to_string());
                                    let sel_mid = loaded
                                        .campaigns
                                        .get(&sel_cid)
                                        .and_then(|c| {
                                            interaction
                                                .active_mission_id
                                                .clone()
                                                .filter(|mid| c.missions.iter().any(|m| &m.id == mid))
                                                .or_else(|| c.missions.first().map(|m| m.id.clone()))
                                        })
                                        .unwrap_or_else(|| "allied_01".to_string());

                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "★ LAUNCH MISSION",
                                        SidebarAction::StartCampaignMission {
                                            campaign_id: sel_cid,
                                            mission_id: sel_mid,
                                        },
                                        true,
                                    );
                                    for (camp_id, camp_def) in loaded.campaigns.iter().take(4) {
                                        let short_lbl = camp_def
                                            .display_name
                                            .split('(')
                                            .next()
                                            .unwrap_or(&camp_def.display_name)
                                            .trim()
                                            .to_uppercase();
                                        spawn_ra2_sidebar_button(
                                            btns,
                                            &short_lbl,
                                            SidebarAction::SelectCampaignById(camp_id.clone()),
                                            false,
                                        );
                                    }
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SKIRMISH BATTLE",
                                        SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "◀ MAIN MENU",
                                        SidebarAction::NavigateMenu(MenuScreen::Root),
                                        false,
                                    );
                                }
                                MenuScreen::Skirmish => {
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "★ LAUNCH SKIRMISH",
                                        SidebarAction::EnterLoadingScreen(format!(
                                            "SKIRMISH — {} ({})",
                                            loaded.entry_map.name, country_name
                                        )),
                                        true,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SWITCH NATION",
                                        SidebarAction::CycleSkirmishCountry,
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "RANDOM PROCEDURAL MAP",
                                        SidebarAction::GenerateProceduralSkirmishMap,
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "CYCLE AI DIFFICULTY",
                                        SidebarAction::CycleAiDifficulty,
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SWITCH GAME MODE",
                                        SidebarAction::CycleGameMode,
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "WEATHER / LIGHTING",
                                        SidebarAction::CycleEnvironmentPreset,
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "◀ MAIN MENU",
                                        SidebarAction::NavigateMenu(MenuScreen::Root),
                                        false,
                                    );
                                }
                                _ => {
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "▶ RESUME BATTLE",
                                        SidebarAction::FinishLoadingEnterGame,
                                        true,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SINGLEPLAYER: CAMPAIGN",
                                        SidebarAction::NavigateMenu(MenuScreen::Campaign),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "SINGLEPLAYER: SKIRMISH",
                                        SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "MULTIPLAYER (ONLINE/LAN)",
                                        SidebarAction::NavigateMenu(MenuScreen::Multiplayer),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "ARCHIVES & REPLAYS",
                                        SidebarAction::NavigateMenu(MenuScreen::Theater),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "UNIT ENCYCLOPEDIA",
                                        SidebarAction::NavigateMenu(MenuScreen::Studio),
                                        false,
                                    );
                                    spawn_ra2_sidebar_button(
                                        btns,
                                        "◀ MAIN MENU",
                                        SidebarAction::NavigateMenu(MenuScreen::Root),
                                        false,
                                    );
                                }
                            }
                        });

                    // 2.3 Bottom RA2 Sidebar Plate (`addon.png`, `44px` height)
                    let addon_h = addon_bg_path
                        .as_deref()
                        .and_then(|p| load_ui_image(asset_server, p));
                    right_col.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(44.0),
                            border: UiRect::top(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(if addon_h.is_some() {
                            Color::WHITE
                        } else {
                            Color::srgb(0.24, 0.05, 0.05)
                        }),
                        BorderColor(Color::srgb(0.86, 0.68, 0.24)),
                        addon_h.map(ImageNode::new).unwrap_or_default(),
                    ));
                });
        });
}

fn spawn_ra2_root_main_menu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let (country_name, faction_id, special_tech, country_load_img) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
    let observer_candidate =
        crate::input::qualify_mod_asset_rel_path(loaded, "assets/ui/loading_screens/observer.png");
    let hero_emblem_path = if std::path::Path::new(&observer_candidate).exists() {
        Some(observer_candidate)
    } else {
        country_load_img.clone()
    };

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|main_stage| {
            // 1. Top Mod Emblem & Faction Dossier Banner
            main_stage
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(170.0),
                        flex_direction: FlexDirection::Row,
                        padding: UiRect::all(Val::Px(8.0)),
                        column_gap: Val::Px(12.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.14, 0.03, 0.03, 0.90)),
                    BorderColor(Color::srgb(0.86, 0.68, 0.24)),
                ))
                .with_children(|hero_row| {
                    if let Some(obs_h) = hero_emblem_path
                        .as_deref()
                        .and_then(|p| load_ui_image(asset_server, p))
                    {
                        hero_row.spawn((
                            ImageNode::new(obs_h),
                            Node {
                                width: Val::Percent(52.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BorderColor(Color::srgb(0.95, 0.80, 0.34)),
                        ));
                    }
                    hero_row
                        .spawn(Node {
                            width: Val::Percent(48.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|meta| {
                            spawn_info_text(
                                meta,
                                loaded.manifest.name.to_uppercase(),
                                Color::srgb(1.0, 0.88, 0.34),
                            );
                            spawn_info_text(
                                meta,
                                format!(
                                    "Active Nation: {} [{}]\nSpecialty Tech: {}\nArsenal: {} Factions | {} Units | {} Buildings | {} Missions",
                                    country_name,
                                    faction_id.to_ascii_uppercase(),
                                    special_tech,
                                    loaded.factions.len(),
                                    loaded.units.len(),
                                    loaded.buildings.len(),
                                    loaded.campaigns.values().map(|c| c.missions.len()).sum::<usize>()
                                ),
                                Color::srgb(0.92, 0.86, 0.76),
                            );
                            spawn_styled_button(
                                meta,
                                &format!("⟳ Switch Active Nation: {}", country_name),
                                SidebarAction::CycleSkirmishCountry,
                                Color::srgb(0.48, 0.12, 0.10),
                                Val::Percent(100.0),
                            );
                        });
                });

            // 2. Center Open Live 3D Battlefield Radar Viewport (`flex_grow: 1.0`, completely transparent!)
            main_stage
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor(Color::srgb(0.84, 0.66, 0.24)),
                ))
                .with_children(|radar_view| {
                    radar_view
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.18, 0.03, 0.03, 0.86)),
                            BorderColor(Color::srgb(0.86, 0.68, 0.24)),
                        ))
                        .with_children(|badge| {
                            badge.spawn((
                                Text::new(studio_layout::sanitize_ui_text(&format!(
                                    "LIVE SATELLITE RECON // {} ({}×{}) — LIGHTING/WEATHER: {}",
                                    loaded.entry_map.name.to_uppercase(),
                                    loaded.entry_map.width,
                                    loaded.entry_map.height,
                                    interaction.environment_preset.label()
                                ))),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.88, 0.36)),
                            ));
                        });
                });

            // 3. Bottom 3 Mod-Driven Theatre Launch Cards (First 2 Story Campaigns from `loaded.campaigns` + Skirmish)
            main_stage
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(210.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|cards| {
                    let mut camp_list: Vec<(&String, &rts_sdk::schema::CampaignDef)> =
                        loaded.campaigns.iter().collect();
                    // Prefer main multi-mission story campaigns before short bootcamp/tutorial campaigns
                    camp_list.sort_by_key(|(id, c)| (c.missions.len() <= 2, id.as_str()));

                    for (camp_id, camp_def) in camp_list.into_iter().take(2) {
                        let [fr, fg, fb] = loaded
                            .factions
                            .get(&camp_def.faction)
                            .map(|f| f.color)
                            .unwrap_or([0.65, 0.82, 1.0]);
                        let card_title = camp_def
                            .display_name
                            .split('(')
                            .next()
                            .unwrap_or(&camp_def.display_name)
                            .trim()
                            .to_uppercase();
                        let camp_img = camp_def
                            .missions
                            .first()
                            .and_then(|m| m.loading_screen.as_deref())
                            .map(|p| crate::input::qualify_mod_asset_rel_path(loaded, p))
                            .or_else(|| {
                                resolve_mod_campaign_loading_screen(loaded, &camp_def.faction, 0)
                            });
                        let first_m_name = camp_def
                            .missions
                            .first()
                            .map(|m| m.display_name.as_str())
                            .unwrap_or("Story Campaign");
                        let summary_line = format!(
                            "{} Missions — Start: {}",
                            camp_def.missions.len(),
                            first_m_name
                        );
                        spawn_column_card(
                            cards,
                            33.3,
                            &card_title,
                            Color::srgb(
                                (fr * 0.7 + 0.3).min(1.0),
                                (fg * 0.7 + 0.3).min(1.0),
                                (fb * 0.7 + 0.3).min(1.0),
                            ),
                            |c| {
                                spawn_4k_artwork_banner(
                                    c,
                                    asset_server,
                                    camp_img.as_deref(),
                                    96.0,
                                );
                                spawn_info_text(
                                    c,
                                    summary_line,
                                    Color::srgb(0.92, 0.90, 0.86),
                                );
                                spawn_ra2_sidebar_button(
                                    c,
                                    "OPEN CAMPAIGN",
                                    SidebarAction::SelectCampaignById(camp_id.clone()),
                                    false,
                                );
                            },
                        );
                    }

                    spawn_column_card(
                        cards,
                        33.4,
                        "SKIRMISH BATTLE",
                        Color::srgb(1.0, 0.88, 0.36),
                        |c| {
                            spawn_4k_artwork_banner(
                                c,
                                asset_server,
                                country_load_img.as_deref(),
                                96.0,
                            );
                            spawn_info_text(
                                c,
                                format!(
                                    "Nation: {} | {} Maps & Seed Generator",
                                    country_name,
                                    loaded.maps.len()
                                ),
                                Color::srgb(0.98, 0.92, 0.78),
                            );
                            spawn_ra2_sidebar_button(
                                c,
                                "CONFIGURE SKIRMISH",
                                SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                                true,
                            );
                        },
                    );
                });
        });
}

fn spawn_info_text(parent: &mut ChildBuilder, text: impl Into<String>, color: Color) {
    let raw: String = text.into();
    parent.spawn((
        Text::new(studio_layout::sanitize_ui_text(&raw)),
        TextFont {
            font_size: 11.5,
            ..default()
        },
        TextColor(color),
    ));
}

fn spawn_loading_screen_view(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let (country_name, faction_id, special_tech, country_load_img) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);

    // Use campaign mission 4K loading screen if a campaign mission is active, otherwise country/faction loading screen
    let active_load_img = if let Some(ref cid) = interaction.active_campaign_id {
        let explicit_screen = interaction
            .active_mission_id
            .as_ref()
            .and_then(|mid| {
                loaded
                    .campaigns
                    .get(cid)
                    .and_then(|c| c.missions.iter().find(|m| &m.id == mid))
                    .and_then(|m| m.loading_screen.as_ref())
                    .map(|p| {
                        if p.starts_with(&loaded.manifest.id) {
                            p.clone()
                        } else {
                            format!("{}/{}", loaded.manifest.id, p)
                        }
                    })
            });
        let m_idx = interaction
            .active_mission_id
            .as_ref()
            .and_then(|mid| {
                loaded
                    .campaigns
                    .get(cid)
                    .and_then(|c| c.missions.iter().position(|m| &m.id == mid))
            })
            .unwrap_or(0);
        explicit_screen
            .or_else(|| resolve_mod_campaign_loading_screen(loaded, &faction_id, m_idx))
            .or(country_load_img)
    } else {
        country_load_img
    };

    let pct = (interaction.loading_progress.clamp(0.08, 1.0) * 100.0).round() as u32;
    let stage_text = if pct < 40 {
        "PHASE 1/3: INITIALIZING 3D TERRAIN MESH, CLIFF TOPOGRAPHY & WATER SHADERS..."
    } else if pct < 85 {
        "PHASE 2/3: LOADING 3D GLB UNITS, TEXTURES & AUDIO STREAMS FROM ACTIVE MOD..."
    } else {
        "PHASE 3/3: DETERMINISTIC LOCKSTEP SIMULATION CORE READY — DEPLOYMENT AUTHORIZED!"
    };

    // Left 62%: Large 4K Remastered Artwork Canvas
    spawn_column_card(
        parent,
        62.0,
        "★ MISSION LOADING SCREEN & PIPELINE PROGRESS ★",
        Color::srgb(0.98, 0.86, 0.42),
        |col| {
            spawn_4k_artwork_banner(col, asset_server, active_load_img.as_deref(), 360.0);
            spawn_stat_bar(
                col,
                stage_text,
                interaction.loading_progress.max(0.12),
                Color::srgb(0.22, 0.88, 0.52),
                &format!("{pct}%"),
            );
            spawn_styled_button(
                col,
                "▶▶ LAUNCH MISSION NOW / ENTER 3D BATTLEFIELD [SPACE / ENTER / ESC] ◀◀",
                SidebarAction::FinishLoadingEnterGame,
                Color::srgb(0.18, 0.54, 0.30),
                Val::Percent(100.0),
            );
        },
    );

    // Right 38%: Tactical Mission Briefing, Faction Cameo Showcase & Telemetry
    spawn_column_card(
        parent,
        38.0,
        "MISSION TELEMETRY & FACTION ARSENAL",
        Color::srgb(0.58, 0.90, 1.0),
        |col| {
            spawn_info_text(
                col,
                format!("OPERATION: {}", interaction.loading_target_title),
                Color::srgb(0.98, 0.92, 0.55),
            );
            spawn_info_text(
                col,
                format!(
                    "FACTION: {} [{}]\nSpecialty Tech: {}",
                    country_name,
                    faction_id.to_ascii_uppercase(),
                    special_tech
                ),
                Color::srgb(0.62, 0.92, 0.82),
            );
            spawn_info_text(
                col,
                format!(
                    "Battlefield Map: {} ({}×{} Tiles, {} Spawns)\nAI Profile: {} ({}) | Fog of War: {} | Weather: {}",
                    loaded.entry_map.name,
                    loaded.entry_map.width,
                    loaded.entry_map.height,
                    loaded.entry_map.spawns.len(),
                    interaction.ai_difficulty.label(),
                    interaction.ai_personality.label(),
                    if interaction.fog_of_war_enabled { "ON" } else { "OFF" },
                    interaction.environment_preset.label()
                ),
                Color::srgb(0.82, 0.88, 0.96),
            );

            // Showcase row of up to 5 HD Cameo Badges dynamically resolved from the active mod's faction units/buildings
            let showcase_ids: Vec<String> = loaded
                .units
                .iter()
                .filter(|(_, u)| u.faction.eq_ignore_ascii_case(&faction_id))
                .map(|(id, _)| id.clone())
                .chain(
                    loaded
                        .buildings
                        .iter()
                        .filter(|(_, b)| b.faction.eq_ignore_ascii_case(&faction_id))
                        .map(|(id, _)| id.clone()),
                )
                .take(5)
                .collect();

            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|cameo_row| {
                for uid in &showcase_ids {
                    if let Some(h) = load_mod_cameo(asset_server, loaded, uid) {
                        cameo_row.spawn((
                            ImageNode::new(h),
                            Node {
                                width: Val::Px(56.0),
                                height: Val::Px(44.0),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BorderColor(Color::srgb(0.95, 0.82, 0.38)),
                        ));
                    }
                }
            });

            spawn_info_text(
                col,
                "BATTLEFIELD COMMAND CONTROLS:\n• [LMB]-Drag: Select Units | [RMB]: Move / Attack\n• [X]: Attack-Move | [G]: Special Ability / Deploy MCV\n• [Z]: Patrol | [F]: Escort / Guard | [ESC]: Tactical Pause Menu",
                Color::srgb(0.90, 0.82, 0.58),
            );
            spawn_styled_button(
                col,
                "◀ Return to Skirmish Configuration",
                SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                Color::srgb(0.24, 0.28, 0.38),
                Val::Percent(100.0),
            );
        },
    );
}

fn spawn_root_main_menu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let (country_name, faction_id, special_tech, country_load_img) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
    let dlg_bg_path = loaded
        .factions
        .get(&faction_id)
        .or_else(|| loaded.factions.values().next())
        .and_then(|f| f.hud_skin.dialog_background.as_deref())
        .map(|p| crate::input::qualify_mod_asset_rel_path(loaded, p));
    let option_count = mod_skirmish_option_count(loaded);
    let fac_names = loaded
        .factions
        .values()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>()
        .join(" / ");
    let campaign_btn_label = if fac_names.is_empty() {
        "1. CAMPAIGN (STORY MISSIONS)".to_string()
    } else {
        format!("1. CAMPAIGN ({fac_names})")
    };

    // 1. Left Glassmorphic Command Blade (33% width) with Mod-Aware Cameo Emblems
    let root_menu_hdr = if interaction.dev_mode_enabled {
        "ENGINE STUDIO — MAIN MENU".to_string()
    } else {
        format!("{} — MAIN MENU", loaded.manifest.name.to_uppercase())
    };
    let studio_btn_label = if interaction.dev_mode_enabled {
        "5. MOD STUDIO, 3D EDITOR & ENCYCLOPEDIA"
    } else {
        "5. 3D UNIT & STRUCTURE ENCYCLOPEDIA"
    };
    spawn_column_card(
        parent,
        33.0,
        &root_menu_hdr,
        Color::srgb(0.98, 0.86, 0.45),
        |col| {
            if let Some(ref bg_rel) = dlg_bg_path {
                if let Some(h) = load_ui_image(asset_server, bg_rel) {
                    col.spawn((
                        ImageNode::new(h).with_color(Color::srgba(1.0, 1.0, 1.0, 0.26)),
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
            }
            spawn_icon_styled_button(
                col,
                &campaign_btn_label,
                load_mod_showcase_cameo(asset_server, loaded, 0),
                SidebarAction::NavigateMenu(MenuScreen::Campaign),
                Color::srgb(0.42, 0.26, 0.14),
                Val::Percent(100.0),
            );
            spawn_icon_styled_button(
                col,
                "2. SKIRMISH (VS. AI COMMANDERS)",
                load_mod_showcase_cameo(asset_server, loaded, 1),
                SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                Color::srgb(0.18, 0.40, 0.28),
                Val::Percent(100.0),
            );
            spawn_icon_styled_button(
                col,
                "3. MULTIPLAYER (rust-rts.bls-isp.net & LAN)",
                load_mod_showcase_cameo(asset_server, loaded, 2),
                SidebarAction::NavigateMenu(MenuScreen::Multiplayer),
                Color::srgb(0.16, 0.38, 0.52),
                Val::Percent(100.0),
            );
            spawn_icon_styled_button(
                col,
                "4. REPLAY THEATER, SAVES & MEDIA ARCHIVE",
                load_mod_showcase_cameo(asset_server, loaded, 3),
                SidebarAction::NavigateMenu(MenuScreen::Theater),
                Color::srgb(0.34, 0.24, 0.46),
                Val::Percent(100.0),
            );
            spawn_icon_styled_button(
                col,
                studio_btn_label,
                load_mod_showcase_cameo(asset_server, loaded, 4),
                SidebarAction::NavigateMenu(MenuScreen::Studio),
                Color::srgb(0.22, 0.32, 0.48),
                Val::Percent(100.0),
            );
            spawn_icon_styled_button(
                col,
                "6. OPTIONS, AUDIO & HOTKEYS",
                load_mod_showcase_cameo(asset_server, loaded, 5),
                SidebarAction::NavigateMenu(MenuScreen::Settings),
                Color::srgb(0.24, 0.28, 0.36),
                Val::Percent(100.0),
            );

            spawn_info_text(
                col,
                format!(
                    "Active Mod: {} (v{}) — {} Factions, {} Units, {} Buildings",
                    loaded.manifest.name,
                    loaded.manifest.version,
                    loaded.factions.len(),
                    loaded.units.len(),
                    loaded.buildings.len()
                ),
                Color::srgb(0.75, 0.84, 0.94),
            );
            spawn_styled_button(
                col,
                "★ QUICKSTART: LAUNCH MISSION",
                SidebarAction::EnterLoadingScreen(format!(
                    "SKIRMISH — {} ({})",
                    loaded.entry_map.name, country_name
                )),
                Color::srgb(0.20, 0.52, 0.30),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "▶ ENTER LIVE 3D BATTLEFIELD DIRECTLY [ESC]",
                SidebarAction::FinishLoadingEnterGame,
                Color::srgb(0.18, 0.38, 0.52),
                Val::Percent(100.0),
            );
        },
    );

    // 2. Center Open 3D Orbital Battlefield Diorama Stage (37% width — completely transparent!)
    parent
        .spawn((
            Node {
                width: Val::Percent(37.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|stage| {
            // Sleek Floating Bottom-Center 3D Diorama Director Bar
            stage
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(12.0)),
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.84)),
                    BorderColor(Color::srgb(0.35, 0.62, 0.92)),
                ))
                .with_children(|director| {
                    spawn_info_text(
                        director,
                        format!(
                            "🎥 LIVE 3D ORBITAL DIORAMA: {} ({}×{}) | Lighting/Weather: {}",
                            loaded.entry_map.name,
                            loaded.entry_map.width,
                            loaded.entry_map.height,
                            interaction.environment_preset.label()
                        ),
                        Color::srgb(0.98, 0.88, 0.48),
                    );
                    director
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_styled_button(
                                row,
                                &format!("🌤 Weather: {}", interaction.environment_preset.label()),
                                SidebarAction::CycleEnvironmentPreset,
                                Color::srgb(0.18, 0.36, 0.44),
                                Val::Percent(48.0),
                            );
                            spawn_styled_button(
                                row,
                                "⟳ Restart 3D Match",
                                SidebarAction::RestartMatch,
                                Color::srgb(0.34, 0.24, 0.18),
                                Val::Percent(52.0),
                            );
                        });
                });
        });

    // 3. Right Floating Faction Showcase & Master-Server Telemetry Card (30% width)
    spawn_column_card(
        parent,
        30.0,
        "FACTION SHOWCASE & ONLINE BACKBONE",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            spawn_4k_artwork_banner(col, asset_server, country_load_img.as_deref(), 165.0);
            spawn_info_text(
                col,
                format!(
                    "{} [{}] — {}",
                    country_name,
                    faction_id.to_ascii_uppercase(),
                    special_tech
                ),
                Color::srgb(0.98, 0.90, 0.52),
            );
            spawn_styled_button(
                col,
                &format!(
                    "⟳ Switch Faction / Sub-Faction ({option_count} available)"
                ),
                SidebarAction::CycleSkirmishCountry,
                Color::srgb(0.24, 0.34, 0.48),
                Val::Percent(100.0),
            );
            spawn_info_text(
                col,
                format!(
                    "Master Registry: {}\nOpen Online Lobbies: {} | Status: {}",
                    interaction.lobby_server_url,
                    interaction.online_lobbies.len(),
                    interaction.net_status_line
                ),
                Color::srgb(0.68, 0.90, 0.98),
            );
            spawn_styled_button(
                col,
                "⟳ Fetch Online Lobbies from rust-rts.bls-isp.net",
                SidebarAction::RefreshOnlineLobbies,
                Color::srgb(0.16, 0.42, 0.50),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "⇄ Switch Target Server (rust-rts.bls-isp.net ↔ Local)",
                SidebarAction::ToggleLobbyServerEndpoint,
                Color::srgb(0.22, 0.28, 0.40),
                Val::Percent(100.0),
            );
        },
    );
}

fn spawn_campaign_submenu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let active_fac_id = if loaded
        .factions
        .contains_key(&interaction.selected_campaign_faction)
    {
        interaction.selected_campaign_faction.clone()
    } else {
        loaded
            .factions
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| interaction.selected_campaign_faction.clone())
    };

    let selected_camp_id = interaction
        .active_campaign_id
        .clone()
        .filter(|cid| loaded.campaigns.contains_key(cid))
        .or_else(|| {
            loaded
                .campaigns
                .iter()
                .find(|(_, c)| c.faction.eq_ignore_ascii_case(&active_fac_id))
                .map(|(id, _)| id.clone())
        })
        .or_else(|| loaded.campaigns.keys().next().cloned())
        .unwrap_or_default();

    let profile = rts_sdk::schema::CampaignSaveProfile::load_from_file(std::path::Path::new(
        "saves/campaign_profile.json",
    ))
    .unwrap_or_default();

    // Column 1: Dynamic Mod-Driven Campaign Selection, Progress & Difficulty
    spawn_column_card(
        parent,
        30.0,
        "2.1 CAMPAIGN SELECTION & PROGRESS",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            let total_missions: usize = loaded.campaigns.values().map(|c| c.missions.len()).sum();
            spawn_info_text(
                col,
                format!(
                    "Select your theatre of war ({} Campaigns, {total_missions} Missions):",
                    loaded.campaigns.len(),
                ),
                Color::srgb(0.82, 0.88, 0.95),
            );
            for (camp_id, camp_def) in &loaded.campaigns {
                let is_sel = camp_id == &selected_camp_id;
                let [r, g, b] = loaded
                    .factions
                    .get(&camp_def.faction)
                    .map(|f| f.color)
                    .unwrap_or([0.4, 0.6, 0.9]);
                let btn_color = if is_sel {
                    Color::srgb(
                        (r * 0.65 + 0.12).min(0.85),
                        (g * 0.65 + 0.12).min(0.85),
                        (b * 0.65 + 0.12).min(0.85),
                    )
                } else {
                    Color::srgb(
                        (r * 0.35 + 0.08).min(0.55),
                        (g * 0.35 + 0.08).min(0.55),
                        (b * 0.35 + 0.08).min(0.55),
                    )
                };
                let first_unit_cameo = loaded
                    .units
                    .iter()
                    .find(|(_, u)| u.faction.eq_ignore_ascii_case(&camp_def.faction))
                    .and_then(|(uid, _)| load_mod_cameo(asset_server, loaded, uid));
                let prefix = if is_sel { "★" } else { "▶" };
                spawn_icon_styled_button(
                    col,
                    &format!("{prefix} {}", camp_def.display_name),
                    first_unit_cameo,
                    SidebarAction::SelectCampaignById(camp_id.clone()),
                    btn_color,
                    Val::Percent(100.0),
                );
            }

            spawn_info_text(
                col,
                format!(
                    "Profile Progress: {} completed | Reserve: +${} | Veteran Carry-Over: {} Units",
                    profile.completed_missions.len(),
                    profile.carry_over_credits,
                    profile.carry_over_units.len()
                ),
                Color::srgb(0.62, 0.94, 0.78),
            );

            spawn_info_text(
                col,
                format!(
                    "Mission Difficulty: {}",
                    interaction.ai_difficulty.label()
                ),
                Color::srgb(0.95, 0.88, 0.55),
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_styled_button(
                    row,
                    "EASY",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Easy),
                    Color::srgb(0.18, 0.38, 0.24),
                    Val::Percent(24.0),
                );
                spawn_styled_button(
                    row,
                    "MED",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Medium),
                    Color::srgb(0.22, 0.34, 0.46),
                    Val::Percent(24.0),
                );
                spawn_styled_button(
                    row,
                    "HARD",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Hard),
                    Color::srgb(0.46, 0.30, 0.14),
                    Val::Percent(25.0),
                );
                spawn_styled_button(
                    row,
                    "BRUTAL",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Brutal),
                    Color::srgb(0.52, 0.16, 0.20),
                    Val::Percent(27.0),
                );
            });
        },
    );

    // Column 2: Paginated Mission Selection List for the Selected Campaign
    spawn_column_card(
        parent,
        36.0,
        "2.2 AVAILABLE STORY MISSIONS",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            if let Some(camp_def) = loaded.campaigns.get(&selected_camp_id) {
                let page_size = 7usize;
                let total = camp_def.missions.len();
                let max_page = total.saturating_sub(1) / page_size;
                let curr_page = interaction.campaign_mission_page.min(max_page);
                let start_idx = curr_page * page_size;

                if max_page > 0 {
                    col.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|nav_row| {
                        spawn_styled_button(
                            nav_row,
                            "◀ Previous Page",
                            SidebarAction::CycleCampaignMissionPage(-1),
                            Color::srgb(0.22, 0.30, 0.44),
                            Val::Percent(48.0),
                        );
                        spawn_styled_button(
                            nav_row,
                            &format!("Page {}/{} ▶", curr_page + 1, max_page + 1),
                            SidebarAction::CycleCampaignMissionPage(1),
                            Color::srgb(0.22, 0.30, 0.44),
                            Val::Percent(52.0),
                        );
                    });
                }

                for miss in camp_def.missions.iter().skip(start_idx).take(page_size) {
                    let is_active = interaction.active_mission_id.as_deref()
                        == Some(miss.id.as_str());
                    let is_done = profile.completed_missions.contains(&miss.id);
                    let badge = if is_active {
                        "★"
                    } else if is_done {
                        "✔"
                    } else {
                        "▶"
                    };
                    let lbl = format!("{badge} {}", miss.display_name);
                    spawn_styled_button(
                        col,
                        &lbl,
                        SidebarAction::StartCampaignMission {
                            campaign_id: selected_camp_id.clone(),
                            mission_id: miss.id.clone(),
                        },
                        if is_active {
                            Color::srgb(0.24, 0.48, 0.28)
                        } else if is_done {
                            Color::srgb(0.18, 0.36, 0.30)
                        } else {
                            Color::srgb(0.20, 0.28, 0.40)
                        },
                        Val::Percent(100.0),
                    );
                }
            }
        },
    );

    // Column 3: Mission Briefing, 4K Loading Artwork & Cutscene/PIP Video Player
    spawn_column_card(
        parent,
        34.0,
        "2.3 MISSION BRIEFING, VIDEOS & OBJECTIVES",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            let active_camp_opt = loaded.campaigns.get(&selected_camp_id);
            let active_miss_opt = active_camp_opt.and_then(|camp| {
                interaction
                    .active_mission_id
                    .as_ref()
                    .and_then(|mid| camp.missions.iter().find(|m| &m.id == mid))
                    .or_else(|| camp.missions.first())
            });

            let preview_screen = active_miss_opt
                .and_then(|m| {
                    m.loading_screen.as_ref().map(|p| {
                        if p.starts_with(&loaded.manifest.id) {
                            p.clone()
                        } else {
                            format!("{}/{}", loaded.manifest.id, p)
                        }
                    })
                })
                .or_else(|| resolve_mod_campaign_loading_screen(loaded, &active_fac_id, 0));
            spawn_4k_artwork_banner(col, asset_server, preview_screen.as_deref(), 150.0);

            if let (Some(camp), Some(miss)) = (active_camp_opt, active_miss_opt) {
                spawn_info_text(
                    col,
                    format!("OPERATION: {}", miss.display_name),
                    Color::srgb(0.98, 0.88, 0.45),
                );
                spawn_info_text(
                    col,
                    format!(
                        "Map: {} | Credit Carry-Over: {:.0}%",
                        miss.map_id,
                        miss.carry_over_credits_ratio * 100.0
                    ),
                    Color::srgb(0.68, 0.86, 1.0),
                );
                spawn_info_text(
                    col,
                    format!("Briefing:\n{}", miss.briefing),
                    Color::srgb(0.86, 0.90, 0.95),
                );

                if miss.intro_video.is_some() || miss.radar_video.is_some() {
                    col.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|vid_row| {
                        if let Some(intro_vid) = &miss.intro_video {
                            spawn_styled_button(
                                vid_row,
                                "🎬 4K Briefing FMV",
                                SidebarAction::PlayModVideo(intro_vid.clone()),
                                Color::srgb(0.42, 0.24, 0.16),
                                Val::Percent(50.0),
                            );
                        }
                        if let Some(radar_vid) = &miss.radar_video {
                            spawn_styled_button(
                                vid_row,
                                "📡 Radar PIP Video",
                                SidebarAction::PlayModVideo(radar_vid.clone()),
                                Color::srgb(0.18, 0.36, 0.48),
                                Val::Percent(50.0),
                            );
                        }
                    });
                }

                spawn_styled_button(
                    col,
                    "▶ LAUNCH MISSION (LOADING SCREEN)",
                    SidebarAction::StartCampaignMission {
                        campaign_id: camp.id.clone(),
                        mission_id: miss.id.clone(),
                    },
                    Color::srgb(0.20, 0.50, 0.30),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "⏭ Advance to Next Campaign Mission",
                    SidebarAction::AdvanceNextCampaignMission,
                    Color::srgb(0.26, 0.34, 0.46),
                    Val::Percent(100.0),
                );
                return;
            }
            spawn_info_text(
                col,
                "Select a campaign mission in Column 2.2 to inspect its briefing, primary objectives, and launch the 3D battlefield.",
                Color::srgb(0.78, 0.84, 0.90),
            );
        },
    );
}

fn spawn_skirmish_submenu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    pid: u8,
) {
    let (country_name, _, special_tech, load_img_path) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);

    // Column 1: Player, Faction/Sub-Faction & AI Slot Setup
    spawn_column_card(
        parent,
        34.0,
        "3.1 PLAYER, FACTION & AI SLOTS",
        Color::srgb(0.98, 0.86, 0.45),
        |col| {
            spawn_styled_button(
                col,
                &format!("Switch Faction / Nation: {} [Click]", country_name),
                SidebarAction::CycleSkirmishCountry,
                Color::srgb(0.22, 0.38, 0.56),
                Val::Percent(100.0),
            );
            spawn_info_text(
                col,
                format!("Faction Specialty: {special_tech}"),
                Color::srgb(0.65, 0.88, 0.98),
            );
            let factions_vec: Vec<_> = loaded.factions.values().collect();
            let slot1_label = factions_vec
                .first()
                .map(|f| format!("Play as {} (Slot 1)", f.name))
                .unwrap_or_else(|| "Play as Player 1 (Slot 1)".to_string());
            let slot2_label = factions_vec
                .get(1)
                .or_else(|| factions_vec.first())
                .map(|f| format!("Play as {} (Slot 2)", f.name))
                .unwrap_or_else(|| "Play as Player 2 (Slot 2)".to_string());
            spawn_icon_styled_button(
                col,
                &slot1_label,
                load_mod_showcase_cameo(asset_server, loaded, 0),
                SidebarAction::SelectPlayerFaction(1),
                Color::srgb(0.16, 0.34, 0.58),
                Val::Percent(100.0),
            );
            spawn_icon_styled_button(
                col,
                &slot2_label,
                load_mod_showcase_cameo(asset_server, loaded, 1),
                SidebarAction::SelectPlayerFaction(2),
                Color::srgb(0.52, 0.16, 0.18),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "Skirmish AI: {} (Click to Toggle)",
                    if interaction.ai_enabled { "ACTIVE" } else { "OFF" }
                ),
                SidebarAction::ToggleSkirmishAi,
                Color::srgb(0.30, 0.22, 0.42),
                Val::Percent(100.0),
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|diff_row| {
                spawn_styled_button(
                    diff_row,
                    "EASY",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Easy),
                    Color::srgb(0.18, 0.38, 0.24),
                    Val::Percent(24.0),
                );
                spawn_styled_button(
                    diff_row,
                    "MED",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Medium),
                    Color::srgb(0.22, 0.34, 0.46),
                    Val::Percent(24.0),
                );
                spawn_styled_button(
                    diff_row,
                    "HARD",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Hard),
                    Color::srgb(0.46, 0.30, 0.14),
                    Val::Percent(25.0),
                );
                spawn_styled_button(
                    diff_row,
                    "BRUTAL",
                    SidebarAction::SetAiDifficulty(AiDifficulty::Brutal),
                    Color::srgb(0.52, 0.16, 0.20),
                    Val::Percent(27.0),
                );
            });
            spawn_styled_button(
                col,
                &format!(
                    "AI Personality: {} [Cycle]",
                    interaction.ai_personality.label()
                ),
                SidebarAction::CycleAiPersonality,
                Color::srgb(0.28, 0.24, 0.44),
                Val::Percent(100.0),
            );
        },
    );

    // Column 2: Faction Artwork Preview, Match Rules, Weather & Launch
    spawn_column_card(
        parent,
        32.0,
        "3.2 FACTION BANNER & LAUNCH",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            spawn_4k_artwork_banner(col, asset_server, load_img_path.as_deref(), 145.0);
            spawn_styled_button(
                col,
                "Switch Game Mode (Standard / Short Game / KotH)",
                SidebarAction::CycleGameMode,
                Color::srgb(0.24, 0.34, 0.44),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "Weather / Time of Day: {} [L]",
                    interaction.environment_preset.label()
                ),
                SidebarAction::CycleEnvironmentPreset,
                Color::srgb(0.20, 0.36, 0.42),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "Fog of War: {} [F5]",
                    if interaction.fog_of_war_enabled { "ON" } else { "OFF" }
                ),
                SidebarAction::ToggleFogOfWar,
                Color::srgb(0.16, 0.34, 0.38),
                Val::Percent(100.0),
            );
            let other_pid = if pid == 1 { 2 } else { 1 };
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|dip_row| {
                spawn_styled_button(
                    dip_row,
                    &format!("ALLY P{other_pid}"),
                    SidebarAction::ToggleAllianceWithPlayer(other_pid),
                    Color::srgb(0.18, 0.38, 0.42),
                    Val::Percent(33.0),
                );
                spawn_styled_button(
                    dip_row,
                    &format!("+500$ -> P{other_pid}"),
                    SidebarAction::TransferCreditsToPlayer(other_pid, 500),
                    Color::srgb(0.38, 0.34, 0.14),
                    Val::Percent(33.0),
                );
                spawn_styled_button(
                    dip_row,
                    &format!("GIFT -> P{other_pid}"),
                    SidebarAction::TransferSelectedUnitsToPlayer(other_pid),
                    Color::srgb(0.32, 0.24, 0.44),
                    Val::Percent(34.0),
                );
            });
            spawn_styled_button(
                col,
                "★ LAUNCH SKIRMISH (WITH FACTION LOADING SCREEN)",
                SidebarAction::EnterLoadingScreen(format!(
                    "SKIRMISH — {} ({})",
                    loaded.entry_map.name, country_name
                )),
                Color::srgb(0.20, 0.52, 0.30),
                Val::Percent(100.0),
            );
        },
    );

    // Column 3: 3D Map Browser & Procedural Seed Map Generator (Live switches 3D Orbital Diorama in background!)
    spawn_column_card(
        parent,
        34.0,
        "3.3 MAP SELECTION & SEED GENERATOR",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            let next_seed = interaction.procedural_map_seed.wrapping_add(1337).max(42);
            spawn_styled_button(
                col,
                &format!("🎲 Generate Procedural Map (Seed #{next_seed})"),
                SidebarAction::GenerateProceduralSkirmishMap,
                Color::srgb(0.42, 0.28, 0.14),
                Val::Percent(100.0),
            );

            let filtered_maps: Vec<(&String, &rts_sdk::schema::MapDef)> = loaded
                .maps
                .iter()
                .filter(|(id, _)| {
                    interaction.skirmish_show_all_maps
                        || (!id.starts_with("ra2_") && !id.starts_with("yr_"))
                })
                .collect();

            let filter_lbl = if interaction.skirmish_show_all_maps {
                format!("⇄ Filter: All Maps ({}) [Click -> Skirmish Only]", loaded.maps.len())
            } else {
                format!("⇄ Filter: Skirmish/MP Maps ({}) [Click -> All {}]", filtered_maps.len(), loaded.maps.len())
            };
            spawn_styled_button(
                col,
                &filter_lbl,
                SidebarAction::ToggleSkirmishMapFilter,
                Color::srgb(0.20, 0.32, 0.44),
                Val::Percent(100.0),
            );

            let page_size = 6usize;
            let total = filtered_maps.len();
            let max_page = total.saturating_sub(1) / page_size;
            let curr_page = interaction.skirmish_map_page.min(max_page);
            let start_idx = curr_page * page_size;

            if max_page > 0 {
                col.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|nav_row| {
                    spawn_styled_button(
                        nav_row,
                        "◀ Prev Page",
                        SidebarAction::CycleSkirmishMapPage(-1),
                        Color::srgb(0.22, 0.30, 0.44),
                        Val::Percent(48.0),
                    );
                    spawn_styled_button(
                        nav_row,
                        &format!("Page {}/{} ▶", curr_page + 1, max_page + 1),
                        SidebarAction::CycleSkirmishMapPage(1),
                        Color::srgb(0.22, 0.30, 0.44),
                        Val::Percent(52.0),
                    );
                });
            }

            for (map_id, map_def) in filtered_maps.into_iter().skip(start_idx).take(page_size) {
                let is_curr = interaction.active_map_id == *map_id
                    || (interaction.active_map_id.is_empty() && map_def.id == loaded.entry_map.id);
                let lbl = if is_curr {
                    format!("★ {} ({}×{})", map_def.name, map_def.width, map_def.height)
                } else {
                    format!("▶ {} ({}×{})", map_def.name, map_def.width, map_def.height)
                };
                spawn_styled_button(
                    col,
                    &lbl,
                    SidebarAction::SwitchActiveMap(map_id.clone()),
                    if is_curr {
                        Color::srgb(0.24, 0.46, 0.36)
                    } else {
                        Color::srgb(0.18, 0.28, 0.38)
                    },
                    Val::Percent(100.0),
                );
            }
        },
    );
}

fn spawn_multiplayer_submenu(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    // Column 1: Online Lobby Browser (`rust-rts.bls-isp.net`)
    spawn_column_card(
        parent,
        38.0,
        "4.1 ONLINE LOBBY BROWSER (rust-rts.bls-isp.net)",
        Color::srgb(0.48, 0.96, 0.78),
        |col| {
            spawn_info_text(
                col,
                format!("Active Registry: {}", interaction.lobby_server_url),
                Color::srgb(0.75, 0.90, 1.0),
            );
            spawn_styled_button(
                col,
                "⇄ Switch Server (rust-rts.bls-isp.net ↔ 127.0.0.1:8080)",
                SidebarAction::ToggleLobbyServerEndpoint,
                Color::srgb(0.24, 0.32, 0.46),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "⟳ Refresh Open Online Lobbies Now",
                SidebarAction::RefreshOnlineLobbies,
                Color::srgb(0.16, 0.42, 0.52),
                Val::Percent(100.0),
            );
            if interaction.online_lobbies.is_empty() {
                spawn_info_text(
                    col,
                    "No open lobbies found. Host your own online lobby in Column 4.2!",
                    Color::srgb(0.78, 0.82, 0.88),
                );
            } else {
                for lob in interaction.online_lobbies.iter().take(6) {
                    let nat_badge = if lob.nat_mode.is_empty() {
                        "Direct"
                    } else {
                        lob.nat_mode.as_str()
                    };
                    let lbl = format!(
                        "▶ [{} {}/{}P | {}] {} — {}",
                        lob.status.label(),
                        lob.current_players,
                        lob.max_players,
                        nat_badge,
                        lob.lobby_name,
                        lob.map_name
                    );
                    spawn_styled_button(
                        col,
                        &lbl,
                        SidebarAction::JoinOnlineLobby(lob.lobby_id.clone()),
                        Color::srgb(0.18, 0.46, 0.30),
                        Val::Percent(100.0),
                    );
                }
            }
        },
    );

    // Column 2: Host New Online Match (UPnP + Reverse-TCP Relay)
    spawn_column_card(
        parent,
        32.0,
        "4.2 HOST NEW ONLINE LOBBY",
        Color::srgb(0.98, 0.86, 0.45),
        |col| {
            spawn_info_text(
                col,
                format!(
                    "Map: {} | Player Slots: {}P",
                    loaded.entry_map.name, interaction.host_player_slots
                ),
                Color::srgb(0.85, 0.92, 0.98),
            );
            spawn_styled_button(
                col,
                &format!(
                    "Cycle Player Slots (Current: {}P -> 2P / 4P / 8P)",
                    interaction.host_player_slots
                ),
                SidebarAction::CycleHostPlayerSlots,
                Color::srgb(0.24, 0.32, 0.46),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "★ HOST MULTIPLAYER LOBBY (UPnP + RELAY TUNNEL)",
                SidebarAction::HostLocalMultiplayer,
                Color::srgb(0.18, 0.48, 0.32),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "📡 Broadcast World-Sync Snapshot to Connected Peers",
                SidebarAction::BroadcastWorldSync,
                Color::srgb(0.32, 0.24, 0.46),
                Val::Percent(100.0),
            );
            spawn_info_text(
                col,
                "Hosting automatically:\n1. Opens TCP port 7878 via UPnP-IGD on your router\n2. Registers a reverse-TCP relay tunnel (ports 27900–27999) for CGNAT/DS-Lite.",
                Color::srgb(0.68, 0.82, 0.90),
            );
        },
    );

    // Column 3: Direct IP Connect & NAT Diagnostics
    spawn_column_card(
        parent,
        30.0,
        "4.3 DIRECT CONNECT & NAT STATUS",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            let ip_lbl = if interaction.ip_input_editing {
                format!("✎ IP:Port [Type + Enter]: {}_", interaction.server_addr_input)
            } else {
                format!(
                    "🌐 Target IP:Port: {} (Click to Edit)",
                    interaction.server_addr_input
                )
            };
            spawn_styled_button(
                col,
                &ip_lbl,
                SidebarAction::ToggleIpInputEditing,
                if interaction.ip_input_editing {
                    Color::srgb(0.45, 0.32, 0.14)
                } else {
                    Color::srgb(0.18, 0.28, 0.40)
                },
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "▶ Connect Directly to IP:Port (Smart-Connect)",
                SidebarAction::JoinLocalMultiplayer,
                Color::srgb(0.20, 0.38, 0.52),
                Val::Percent(100.0),
            );
            spawn_info_text(
                col,
                format!(
                    "Active Network Status:\n{}\nActive Lobby ID: {}",
                    interaction.net_status_line,
                    interaction.active_lobby_id.as_deref().unwrap_or("None")
                ),
                Color::srgb(0.58, 0.92, 0.76),
            );
        },
    );
}

fn spawn_theater_submenu(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    save_files: &[PathBuf],
    replay_files: &[PathBuf],
) {
    // Column 1: 3D Replay Theater
    spawn_column_card(
        parent,
        34.0,
        "5.1 3D REPLAY THEATER (.replay.json)",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            spawn_styled_button(
                col,
                "🎬 Play Last Match as 3D Replay (Observer)",
                SidebarAction::StartReplayPlayback,
                Color::srgb(0.42, 0.32, 0.16),
                Val::Percent(100.0),
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_styled_button(
                    row,
                    "⏪ -100 Ticks",
                    SidebarAction::SeekReplayRelative(-100),
                    Color::srgb(0.24, 0.30, 0.42),
                    Val::Percent(33.0),
                );
                spawn_styled_button(
                    row,
                    "⏩ +100 Ticks",
                    SidebarAction::SeekReplayRelative(100),
                    Color::srgb(0.24, 0.30, 0.42),
                    Val::Percent(33.0),
                );
                spawn_styled_button(
                    row,
                    "👁 Cycle POV",
                    SidebarAction::CycleReplayObserverPov,
                    Color::srgb(0.28, 0.24, 0.44),
                    Val::Percent(34.0),
                );
            });
            for rp in replay_files.iter().take(6) {
                let fname = rp
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("replay.json");
                spawn_styled_button(
                    col,
                    &format!("🎬 Load Replay: {fname}"),
                    SidebarAction::PlayReplayFile(rp.clone()),
                    Color::srgb(0.34, 0.26, 0.16),
                    Val::Percent(100.0),
                );
            }
        },
    );

    // Column 2: Savegames Manager
    spawn_column_card(
        parent,
        33.0,
        "5.2 SAVEGAME ARCHIVE",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            spawn_styled_button(
                col,
                "💾 Quick-Save Current Battle [F7]",
                SidebarAction::QuickSaveMatch,
                Color::srgb(0.18, 0.42, 0.28),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "📂 Quick-Load Last Savegame [F8]",
                SidebarAction::QuickLoadMatch,
                Color::srgb(0.20, 0.36, 0.46),
                Val::Percent(100.0),
            );
            for sp in save_files.iter().take(6) {
                let fname = sp
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("save.json");
                spawn_styled_button(
                    col,
                    &format!("💾 Load Savegame: {fname}"),
                    SidebarAction::LoadSaveFile(sp.clone()),
                    Color::srgb(0.18, 0.32, 0.28),
                    Val::Percent(100.0),
                );
            }
        },
    );

    // Column 3: Dynamic Mod Media, Soundtrack Jukebox & 4K Cutscene Archive
    spawn_column_card(
        parent,
        33.0,
        "5.3 MOD ARCHIVE, FMV THEATER & JUKEBOX",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            spawn_info_text(
                col,
                format!(
                    "Loaded Mod Media Library: {} ({} v{})\n• {} Factions | {} Units | {} Buildings\n• {} Campaigns ({} Maps)",
                    loaded.manifest.name,
                    loaded.manifest.id,
                    loaded.manifest.version,
                    loaded.factions.len(),
                    loaded.units.len(),
                    loaded.buildings.len(),
                    loaded.campaigns.len(),
                    loaded.maps.len(),
                ),
                Color::srgb(0.82, 0.88, 0.95),
            );
            if !loaded.audio.music_tracks.is_empty() {
                spawn_info_text(
                    col,
                    "🎵 SOUNDTRACK JUKEBOX:",
                    Color::srgb(0.98, 0.86, 0.45),
                );
                for track in loaded.audio.music_tracks.iter().take(4) {
                    spawn_styled_button(
                        col,
                        &format!("🎵 Play Track: {}", track.display_name),
                        SidebarAction::PlayModMusicTrack(track.id.clone()),
                        Color::srgb(0.22, 0.34, 0.48),
                        Val::Percent(100.0),
                    );
                }
            }
            spawn_info_text(
                col,
                "🎬 4K FMV CUTSCENE ARCHIVE:",
                Color::srgb(0.98, 0.86, 0.45),
            );
            for (label, rel_vid) in [
                ("🎬 RA2 Opening Intro (4K)", "assets/videos/ra2_opening_intro.mp4"),
                ("🎬 Yuri's Revenge Intro (4K)", "assets/videos/yr_opening_intro.mp4"),
                ("🎬 Allied Campaign Opener (4K)", "assets/videos/allied_01_cutscene.mp4"),
                ("🎬 Soviet Campaign Opener (4K)", "assets/videos/soviet_01_cutscene.mp4"),
            ] {
                if loaded.resolve_asset_path(rel_vid).exists() {
                    spawn_styled_button(
                        col,
                        label,
                        SidebarAction::PlayModVideo(rel_vid.to_string()),
                        Color::srgb(0.38, 0.24, 0.16),
                        Val::Percent(100.0),
                    );
                }
            }
            if let Some(active_vid) = &interaction.active_video_preview_path {
                spawn_info_text(
                    col,
                    format!("Last Played Video: {active_vid}"),
                    Color::srgb(0.58, 0.92, 0.76),
                );
            }
        },
    );
}

fn spawn_studio_and_encyclopedia_submenu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    _asset_root: &AssetRootResource,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    discovered_mods: &[PathBuf],
) {
    let active_fac_id = if loaded
        .factions
        .contains_key(&interaction.selected_encyclopedia_faction)
    {
        interaction.selected_encyclopedia_faction.clone()
    } else {
        loaded
            .factions
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| interaction.selected_encyclopedia_faction.clone())
    };

    // Column 1: Mod Manager & Asset Pipeline Converter (Only visible in Engine Developer Mode!)
    if interaction.dev_mode_enabled {
        spawn_column_card(
            parent,
            30.0,
            "6.1 MOD MANAGER & CONVERTER",
            Color::srgb(0.98, 0.86, 0.45),
            |col| {
                for mod_dir in discovered_mods {
                    let folder_label = mod_dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("mod");
                    let is_active = folder_label == loaded.manifest.id;
                    let btn_label = if is_active {
                        format!("★ Active: {folder_label} (Reload)")
                    } else {
                        format!("▶ Load Mod: {folder_label}")
                    };
                    spawn_styled_button(
                        col,
                        &btn_label,
                        SidebarAction::LoadModFromPath(mod_dir.clone()),
                        if is_active {
                            Color::srgb(0.20, 0.44, 0.28)
                        } else {
                            Color::srgb(0.20, 0.28, 0.42)
                        },
                        Val::Percent(100.0),
                    );
                }
                spawn_styled_button(
                    col,
                    "⚡ HOT-RELOAD ACTIVE MOD LIVE [Ctrl+F12]",
                    SidebarAction::HotReloadMod,
                    Color::srgb(0.16, 0.46, 0.32),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "✔ Validate Active Mod & Assets",
                    SidebarAction::ValidateActiveMod,
                    Color::srgb(0.18, 0.34, 0.44),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "🔨 Regenerate 3D .glb Models",
                    SidebarAction::RegenerateActiveModGlb,
                    Color::srgb(0.24, 0.30, 0.46),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "⚙ Convert Westwood & OpenRA Demo Packs",
                    SidebarAction::ConvertDemoPacks,
                    Color::srgb(0.46, 0.30, 0.14),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "🔍 Auto-Scan & Import 'legacy-imports/'",
                    SidebarAction::AutoScanLegacyImportsDir,
                    Color::srgb(0.20, 0.38, 0.32),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    &format!(
                        "🖌 Editor Brush Size: {}",
                        interaction.map_editor_brush_size.label()
                    ),
                    SidebarAction::CycleEditorBrushSize,
                    Color::srgb(0.22, 0.34, 0.48),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    &format!(
                        "🔄 Editor Symmetry: {}",
                        interaction.map_editor_symmetry.label()
                    ),
                    SidebarAction::CycleEditorSymmetry,
                    Color::srgb(0.26, 0.32, 0.48),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "🗺 Check Map Playability (A* & Spawns)",
                    SidebarAction::ValidateMapPlayability,
                    Color::srgb(0.22, 0.42, 0.34),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "⚖ Run Headless Balance Benchmark & Outlier Check",
                    SidebarAction::RunBalanceBenchmark,
                    Color::srgb(0.38, 0.28, 0.16),
                    Val::Percent(100.0),
                );
            },
        );
    }

    let list_col_w = if interaction.dev_mode_enabled { 36.0 } else { 50.0 };
    let dossier_col_w = if interaction.dev_mode_enabled { 34.0 } else { 50.0 };

    // Column 2: Dynamic Mod-Driven 3D Unit & Tech-Tree Encyclopedia Browser
    spawn_column_card(
        parent,
        list_col_w,
        "3D UNIT & STRUCTURE ENCYCLOPEDIA",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            let fac_count = loaded.factions.len().max(1);
            let tab_w = (100.0 / (fac_count as f32)).floor();
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for (fac_id, fac_def) in &loaded.factions {
                    let is_sel = fac_id.eq_ignore_ascii_case(&active_fac_id);
                    let [r, g, b] = fac_def.color;
                    let tab_color = if is_sel {
                        Color::srgb(
                            (r * 0.65 + 0.12).min(0.85),
                            (g * 0.65 + 0.12).min(0.85),
                            (b * 0.65 + 0.12).min(0.85),
                        )
                    } else {
                        Color::srgb(
                            (r * 0.35 + 0.08).min(0.55),
                            (g * 0.35 + 0.08).min(0.55),
                            (b * 0.35 + 0.08).min(0.55),
                        )
                    };
                    spawn_styled_button(
                        row,
                        &fac_def.name.to_uppercase(),
                        SidebarAction::SelectEncyclopediaFaction(fac_id.clone()),
                        tab_color,
                        Val::Percent(tab_w),
                    );
                }
            });

            let fac = active_fac_id.as_str();
            for (u_id, u_def) in loaded
                .units
                .iter()
                .filter(|(_, u)| u.faction.eq_ignore_ascii_case(fac))
                .take(8)
            {
                let is_sel =
                    interaction.selected_encyclopedia_entry.as_deref() == Some(u_id.as_str());
                let cameo = load_mod_cameo(asset_server, loaded, u_id);
                spawn_icon_styled_button(
                    col,
                    &format!("{} (${}, {:.0} HP)", u_def.name, u_def.cost, u_def.health),
                    cameo,
                    SidebarAction::InspectEncyclopediaEntry(u_id.clone()),
                    if is_sel {
                        Color::srgb(0.24, 0.48, 0.34)
                    } else {
                        Color::srgb(0.18, 0.26, 0.38)
                    },
                    Val::Percent(100.0),
                );
            }
        },
    );

    // Column 3: Detailed 3D Unit Dossier with Large Cameo Portrait & Visual Stat Bars
    spawn_column_card(
        parent,
        dossier_col_w,
        if interaction.dev_mode_enabled {
            "6.3 3D DOSSIER, CAMEO & MAP EDITOR"
        } else {
            "UNIT DOSSIER & TECHNICAL SPECIFICATION"
        },
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            let active_entry = interaction
                .selected_encyclopedia_entry
                .clone()
                .filter(|id| loaded.units.contains_key(id) || loaded.buildings.contains_key(id))
                .or_else(|| {
                    loaded
                        .units
                        .iter()
                        .find(|(_, u)| u.faction.eq_ignore_ascii_case(&active_fac_id))
                        .map(|(k, _)| k.clone())
                });

            if let Some(entry_id) = active_entry {
                if let Some(u) = loaded.units.get(&entry_id) {
                    col.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|hdr| {
                        if let Some(h) = load_mod_cameo(asset_server, loaded, &entry_id) {
                            hdr.spawn((
                                ImageNode::new(h),
                                Node {
                                    width: Val::Px(76.0),
                                    height: Val::Px(60.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BorderColor(Color::srgb(0.98, 0.85, 0.40)),
                            ));
                        }
                        hdr.spawn((
                            Text::new(studio_layout::sanitize_ui_text(&format!(
                                "{}\nID: {} | Armor: {:?}",
                                u.name, u.id, u.armor
                            ))),
                            TextFont {
                                font_size: 12.5,
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.92, 0.55)),
                        ));
                    });

                    spawn_stat_bar(
                        col,
                        "Structure / Hit Points (HP)",
                        u.health / 1000.0,
                        Color::srgb(0.22, 0.86, 0.45),
                        &format!("{:.0} HP", u.health),
                    );
                    spawn_stat_bar(
                        col,
                        "Speed & Mobility",
                        u.speed / 12.0,
                        Color::srgb(0.28, 0.75, 0.98),
                        &format!("{:.1} m/s ({:?})", u.speed, u.movement_domain),
                    );
                    spawn_stat_bar(
                        col,
                        "Production Cost",
                        (u.cost as f32) / 2000.0,
                        Color::srgb(0.95, 0.78, 0.28),
                        &format!("${}", u.cost),
                    );

                    let wp = u
                        .primary_weapon
                        .as_ref()
                        .and_then(|w| loaded.weapons.get(w))
                        .map(|w| format!("{} ({:.0} DMG, {:.0}m)", w.name, w.damage, w.range))
                        .unwrap_or_else(|| "None".to_string());
                    spawn_info_text(
                        col,
                        format!(
                            "Primary Weapon: {}\n3D Asset: {}\nTech Tier: {}",
                            wp,
                            u.visual.model,
                            if u.prerequisites.is_empty() {
                                "Base Unit".to_string()
                            } else {
                                u.prerequisites.join(" + ")
                            }
                        ),
                        Color::srgb(0.84, 0.90, 0.98),
                    );
                }
            }
            if interaction.dev_mode_enabled {
                spawn_styled_button(
                    col,
                    &format!(
                        "🖌 3D Map Editor Brush: {} [F10]",
                        interaction.map_editor_brush.label()
                    ),
                    SidebarAction::CycleMapEditorBrush,
                    Color::srgb(0.36, 0.24, 0.44),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    "💾 Export Edited 3D Map as .toml [F11]",
                    SidebarAction::ExportEditedMap,
                    Color::srgb(0.22, 0.40, 0.30),
                    Val::Percent(100.0),
                );
            }
        },
    );
}

fn spawn_settings_submenu(parent: &mut ChildBuilder, interaction: &ClientInteractionState) {
    // Column 1: Audio & Camera
    spawn_column_card(
        parent,
        34.0,
        "7.1 AUDIO, EVA & 3D CAMERA",
        Color::srgb(0.98, 0.86, 0.45),
        |col| {
            spawn_styled_button(
                col,
                "🔊 Cycle & Save Master Volume",
                SidebarAction::CycleMasterVolume,
                Color::srgb(0.22, 0.38, 0.32),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "🎥 Cycle & Save Camera Pan Speed ({:.0})",
                    interaction.camera_pan_speed
                ),
                SidebarAction::CycleCameraPanSpeed,
                Color::srgb(0.24, 0.34, 0.44),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "🌤 Weather / Lighting Preset: {} [L]",
                    interaction.environment_preset.label()
                ),
                SidebarAction::CycleEnvironmentPreset,
                Color::srgb(0.20, 0.36, 0.42),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "🌫 Default Fog of War: {} [F5]",
                    if interaction.fog_of_war_enabled { "ON" } else { "OFF" }
                ),
                SidebarAction::ToggleFogOfWar,
                Color::srgb(0.16, 0.34, 0.38),
                Val::Percent(100.0),
            );
        },
    );

    // Column 2: Hotkeys & Tactical Controls
    spawn_column_card(
        parent,
        33.0,
        "7.2 KEYBINDINGS (saves/settings.toml)",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            spawn_info_text(
                col,
                format!(
                    "Active Hotkeys:\n• Attack-Move: [{}]\n• Stop Units: [{}]\n• Scatter Units: [{}]\n• Special Ability / Deploy: [{}]\n• Rotate Building: [{}]\n• Control Groups: [Ctrl + 1..5] / [1..5]\n• Camera Bookmarks: [Ctrl + F1..F4]",
                    interaction.hotkey_attack_move,
                    interaction.hotkey_stop,
                    interaction.hotkey_scatter,
                    interaction.hotkey_activate_ability,
                    interaction.hotkey_rotate_building
                ),
                Color::srgb(0.84, 0.90, 0.96),
            );
        },
    );

    // Column 3: Network & Master Server Settings
    spawn_column_card(
        parent,
        33.0,
        "7.3 NETWORK & MASTER SERVER",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            spawn_info_text(
                col,
                format!(
                    "Central Multiplayer Registry:\n{}\nDefault Lockstep Port: {}\nRelay Port Range: TCP 27900–27999",
                    interaction.lobby_server_url, interaction.server_addr_input
                ),
                Color::srgb(0.82, 0.90, 0.96),
            );
            spawn_styled_button(
                col,
                "⇄ Switch Master Server (rust-rts.bls-isp.net ↔ Local)",
                SidebarAction::ToggleLobbyServerEndpoint,
                Color::srgb(0.22, 0.34, 0.48),
                Val::Percent(100.0),
            );
        },
    );
}

fn spawn_pause_menu_submenu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let (country_name, _, _, load_img_path) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);

    spawn_column_card(
        parent,
        50.0,
        "TACTICAL PAUSE MENU (IN-GAME)",
        Color::srgb(0.98, 0.86, 0.45),
        |col| {
            spawn_styled_button(
                col,
                "▶ RESUME BATTLE [ESC]",
                SidebarAction::FinishLoadingEnterGame,
                Color::srgb(0.18, 0.48, 0.28),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "💾 QUICK-SAVE MATCH [F7]",
                SidebarAction::QuickSaveMatch,
                Color::srgb(0.18, 0.36, 0.44),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "📂 QUICK-LOAD MATCH [F8]",
                SidebarAction::QuickLoadMatch,
                Color::srgb(0.22, 0.34, 0.46),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "⟳ RESTART MISSION / SKIRMISH",
                SidebarAction::RestartMatch,
                Color::srgb(0.42, 0.28, 0.16),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "⚙ OPTIONS & AUDIO SETTINGS",
                SidebarAction::NavigateMenu(MenuScreen::Settings),
                Color::srgb(0.24, 0.30, 0.40),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "◀ ABORT BATTLE & RETURN TO MAIN MENU",
                SidebarAction::ReturnToMainMenu,
                Color::srgb(0.52, 0.18, 0.20),
                Val::Percent(100.0),
            );
        },
    );

    spawn_column_card(
        parent,
        50.0,
        "CURRENT MISSION & BATTLEFIELD STATUS",
        Color::srgb(0.68, 0.88, 1.0),
        |col| {
            spawn_4k_artwork_banner(col, asset_server, load_img_path.as_deref(), 165.0);
            spawn_info_text(
                col,
                format!(
                    "Mod: {} (v{}) | Faction: {}\nMap: {} ({}×{})\nNetwork Mode: {}\nAI Difficulty: {} ({})",
                    loaded.manifest.name,
                    loaded.manifest.version,
                    country_name,
                    loaded.entry_map.name,
                    loaded.entry_map.width,
                    loaded.entry_map.height,
                    interaction.net_status_line,
                    interaction.ai_difficulty.label(),
                    interaction.ai_personality.label()
                ),
                Color::srgb(0.84, 0.90, 0.96),
            );
        },
    );
}
