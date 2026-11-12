pub mod campaign;
pub mod encyclopedia;
pub mod main_menu;
pub mod mp_staging_room;
pub mod mp_staging_rules;
pub mod multiplayer;
pub mod pause;
pub mod post_game_graphs;
pub mod post_game_score;
pub mod settings;
pub mod skirmish;
pub mod theater;
pub mod workshop_screen;

use crate::audio::SoundSettingsResource;
use crate::input::{
    resolve_mod_campaign_loading_screen, resolve_mod_skirmish_descriptor, AppPhase,
    ClientInteractionState, MenuScreen, UiAssetCache,
};
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

pub fn spawn_screen_card(
    parent: &mut ChildBuilder,
    width_pct: f32,
    title: &str,
    accent: Color,
    build: impl FnOnce(&mut ChildBuilder),
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(width_pct),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.11, 0.92)),
            BorderColor(accent),
        ))
        .with_children(|col| {
            col.spawn((
                Text::new(sanitize_ui_text(title)),
                TextFont {
                    font_size: 13.5,
                    ..default()
                },
                TextColor(accent),
            ));
            build(col);
        });
}

pub fn spawn_body_text(parent: &mut ChildBuilder, text: impl AsRef<str>, color: Color) {
    parent.spawn((
        Text::new(sanitize_ui_text(text.as_ref())),
        TextFont {
            font_size: 11.5,
            ..default()
        },
        TextColor(color),
    ));
}

pub fn spawn_artwork_banner(
    parent: &mut ChildBuilder,
    ui_cache: &mut UiAssetCache,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    qualified_path: Option<&str>,
    height_px: f32,
) {
    let img_opt =
        qualified_path.and_then(|p| ui_cache.load_exact_image(asset_server, &asset_root.0, p));

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height_px),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.03, 0.05, 0.09)),
            BorderColor(Color::srgb(0.38, 0.54, 0.78)),
        ))
        .with_children(|frame| {
            if let Some(handle) = img_opt {
                frame.spawn((
                    ImageNode::new(handle),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
            } else {
                frame.spawn((
                    Text::new("TACTICAL SECTOR SATELLITE UPLINK ACTIVE"),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.55, 0.74, 0.92)),
                ));
            }
        });
}

#[allow(clippy::too_many_arguments)]
pub fn populate_active_menu_screen(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    ui_font: Option<&UiFontResource>,
    sound_res: &SoundSettingsResource,
    video_res: Option<&VideoPlaybackResource>,
    post_game_graph: Option<Handle<Image>>,
    world: &rts_simulation::world::SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    discovered_mods: &[PathBuf],
    save_files: &[PathBuf],
    replay_files: &[PathBuf],
    pid: u8,
) {
    if interaction.app_phase == AppPhase::LoadingScreen {
        spawn_loading_screen_view(
            parent,
            asset_server,
            asset_root,
            ui_cache,
            loaded,
            interaction,
        );
        return;
    }

    match interaction.active_menu_screen {
        MenuScreen::Root => main_menu::spawn_main_menu(
            parent,
            asset_server,
            asset_root,
            ui_cache,
            loaded,
            interaction,
            discovered_mods,
        ),
        MenuScreen::Campaign => campaign::spawn_campaign_screen(
            parent,
            asset_server,
            asset_root,
            ui_cache,
            loaded,
            interaction,
        ),
        MenuScreen::Skirmish => skirmish::spawn_skirmish_screen(
            parent,
            asset_server,
            asset_root,
            ui_cache,
            ui_font,
            loaded,
            interaction,
            pid,
        ),
        MenuScreen::Multiplayer => {
            multiplayer::spawn_multiplayer_screen(parent, ui_font, loaded, interaction)
        }
        MenuScreen::MultiplayerStaging => mp_staging_room::spawn_mp_staging_room_screen(
            parent,
            asset_server,
            asset_root,
            ui_cache,
            ui_font,
            loaded,
            interaction,
            save_files,
        ),
        MenuScreen::ModWorkshop => workshop_screen::spawn_workshop_screen(
            parent,
            ui_font,
            loaded,
            interaction,
            discovered_mods,
        ),
        MenuScreen::Theater => theater::spawn_theater_screen(
            parent,
            loaded,
            interaction,
            video_res,
            save_files,
            replay_files,
        ),
        MenuScreen::Encyclopedia | MenuScreen::Studio => encyclopedia::spawn_encyclopedia_screen(
            parent,
            asset_server,
            asset_root,
            ui_cache,
            loaded,
            interaction,
        ),
        MenuScreen::Settings => {
            settings::spawn_settings_screen(parent, ui_font, loaded, interaction, sound_res)
        }
        MenuScreen::PauseMenu => pause::spawn_pause_screen(parent, loaded, interaction),
        MenuScreen::PostGameScore => post_game_score::spawn_post_game_score_screen(
            parent,
            loaded,
            world,
            interaction,
            video_res,
            post_game_graph,
        ),
    }
}

fn spawn_loading_screen_view(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let (_, fac_id, desc, country_art) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
    let art_path = country_art.or_else(|| resolve_mod_campaign_loading_screen(loaded, &fac_id, 0));

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(24.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.96)),
            BorderColor(Color::srgb(0.86, 0.72, 0.28)),
        ))
        .with_children(|col| {
            col.spawn((
                Text::new(sanitize_ui_text(&format!(
                    "DEPLOYING COMMANDER — {}",
                    interaction.loading_target_title
                ))),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.88, 0.45)),
            ));

            spawn_artwork_banner(
                col,
                ui_cache,
                asset_server,
                asset_root,
                art_path.as_deref(),
                400.0,
            );

            spawn_body_text(col, desc, Color::srgb(0.84, 0.90, 0.96));

            let pct = (interaction.loading_progress * 100.0).clamp(15.0, 100.0);
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.09, 0.14)),
                BorderColor(Color::srgb(0.42, 0.62, 0.88)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent(pct),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.24, 0.88, 0.48)),
                ));
            });

            spawn_styled_button(
                col,
                "ENTER LIVE 3D BATTLEFIELD [SPACE / ENTER]",
                SidebarAction::FinishLoadingEnterGame,
                Color::srgb(0.18, 0.48, 0.28),
                Val::Percent(100.0),
            );
        });
}
