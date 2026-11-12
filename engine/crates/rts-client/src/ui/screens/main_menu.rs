use super::{spawn_artwork_banner, spawn_body_text};
use crate::input::{
    resolve_mod_skirmish_descriptor, ClientInteractionState, MenuScreen, UiAssetCache,
};
use crate::ui::locale::ui_t;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

pub fn spawn_main_menu(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    discovered_mods: &[PathBuf],
) {
    parent
        .spawn((
            Node {
                width: Val::Px(390.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.90)),
            BorderColor(Color::srgb(0.84, 0.68, 0.28)),
        ))
        .with_children(|blade| {
            blade
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|top| {
                    top.spawn((
                        Text::new(sanitize_ui_text(&loaded.manifest.name.to_uppercase())),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.98, 0.88, 0.44)),
                    ));
                    spawn_body_text(
                        top,
                        format!(
                            "Mod: {} v{}  |  {} Factions  |  {} Units",
                            loaded.manifest.id,
                            loaded.manifest.version,
                            loaded.factions.len(),
                            loaded.units.len()
                        ),
                        Color::srgb(0.72, 0.84, 0.96),
                    );

                    let (_, _, _, art) =
                        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
                    spawn_artwork_banner(
                        top,
                        ui_cache,
                        asset_server,
                        asset_root,
                        art.as_deref(),
                        126.0,
                    );

                    // Crash-Reconnect Banner (`Task 248-UI`) when `saves/reconnect_session.json` exists
                    if let Ok(Some(saved_sess)) =
                        rts_network::reconnect_store::ReconnectSessionInfo::load_default()
                    {
                        top.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.18, 0.14, 0.04, 0.96)),
                            BorderColor(Color::srgb(1.0, 0.84, 0.28)),
                        ))
                        .with_children(|rec| {
                            spawn_body_text(
                                rec,
                                format!(
                                    "ACTIVE MATCH FOUND: Slot P{} ({}) @ {}",
                                    saved_sess.player_id,
                                    saved_sess.player_name,
                                    saved_sess.server_endpoint
                                ),
                                Color::srgb(1.0, 0.92, 0.48),
                            );
                            rec.spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|rbtns| {
                                spawn_styled_button(
                                    rbtns,
                                    &ui_t(loaded, interaction, "ui.menu.rejoin_match"),
                                    SidebarAction::RejoinSavedMatch,
                                    Color::srgb(0.20, 0.54, 0.30),
                                    Val::Percent(80.0),
                                );
                                spawn_styled_button(
                                    rbtns,
                                    "X",
                                    SidebarAction::DismissSavedReconnect,
                                    Color::srgb(0.46, 0.18, 0.20),
                                    Val::Percent(20.0),
                                );
                            });
                        });
                    }

                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.bootcamp"),
                        SidebarAction::LaunchBootcampTutorial,
                        Color::srgb(0.46, 0.34, 0.12),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.campaign"),
                        SidebarAction::NavigateMenu(MenuScreen::Campaign),
                        Color::srgb(0.44, 0.24, 0.14),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.skirmish"),
                        SidebarAction::NavigateMenu(MenuScreen::Skirmish),
                        Color::srgb(0.16, 0.42, 0.26),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.multiplayer"),
                        SidebarAction::NavigateMenu(MenuScreen::Multiplayer),
                        Color::srgb(0.16, 0.36, 0.50),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.workshop"),
                        SidebarAction::NavigateMenu(MenuScreen::ModWorkshop),
                        Color::srgb(0.20, 0.40, 0.48),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.theater"),
                        SidebarAction::NavigateMenu(MenuScreen::Theater),
                        Color::srgb(0.32, 0.22, 0.44),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.encyclopedia"),
                        SidebarAction::NavigateMenu(MenuScreen::Encyclopedia),
                        Color::srgb(0.22, 0.30, 0.46),
                        Val::Percent(100.0),
                    );
                    spawn_styled_button(
                        top,
                        &ui_t(loaded, interaction, "ui.menu.options"),
                        SidebarAction::NavigateMenu(MenuScreen::Settings),
                        Color::srgb(0.22, 0.26, 0.34),
                        Val::Percent(100.0),
                    );
                });

            blade
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|footer| {
                    if !interaction.lock_player_mode && discovered_mods.len() > 1 {
                        spawn_body_text(
                            footer,
                            "Switch Installed Mod:",
                            Color::srgb(0.76, 0.84, 0.94),
                        );
                        footer
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(4.0),
                                row_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|mod_row| {
                                for mpath in discovered_mods.iter().take(4) {
                                    let folder =
                                        mpath.file_name().and_then(|s| s.to_str()).unwrap_or("mod");
                                    let active = folder == loaded.manifest.id;
                                    spawn_styled_button(
                                        mod_row,
                                        folder,
                                        SidebarAction::LoadModFromPath(mpath.clone()),
                                        if active {
                                            Color::srgb(0.24, 0.48, 0.32)
                                        } else {
                                            Color::srgb(0.16, 0.22, 0.32)
                                        },
                                        Val::Percent(48.5),
                                    );
                                }
                            });
                    }

                    if !interaction.lock_player_mode {
                        spawn_styled_button(
                            footer,
                            &ui_t(loaded, interaction, "ui.menu.workbench"),
                            SidebarAction::OpenCreatorWorkbench,
                            Color::srgb(0.26, 0.32, 0.44),
                            Val::Percent(100.0),
                        );
                    }
                    spawn_styled_button(
                        footer,
                        &ui_t(loaded, interaction, "ui.menu.exit"),
                        SidebarAction::ExitApplication,
                        Color::srgb(0.46, 0.14, 0.16),
                        Val::Percent(100.0),
                    );
                });
        });
}
