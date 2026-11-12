use super::{spawn_body_text, spawn_screen_card};
use crate::input::{ClientInteractionState, MenuScreen};
use crate::ui::locale::ui_t;
use crate::ui::player_windows::PlayerSubWindowKind;
use crate::ui::studio_layout::spawn_styled_button;
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;

pub fn spawn_pause_screen(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|wrap| {
            let title = ui_t(loaded, interaction, "ui.pause.title");
            spawn_screen_card(wrap, 44.0, &title, Color::srgb(0.98, 0.84, 0.42), |col| {
                spawn_body_text(
                    col,
                    format!(
                        "Active Sector: {}  |  Mod: {}",
                        loaded.entry_map.name, loaded.manifest.name
                    ),
                    Color::srgb(0.78, 0.88, 0.96),
                );

                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.pause.resume"),
                    SidebarAction::FinishLoadingEnterGame,
                    Color::srgb(0.18, 0.48, 0.28),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.pause.briefing"),
                    SidebarAction::OpenPlayerWindowFromMenu(PlayerSubWindowKind::Briefing),
                    Color::srgb(0.24, 0.34, 0.48),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.pause.save_load"),
                    SidebarAction::OpenPlayerWindowFromMenu(PlayerSubWindowKind::SaveLoad),
                    Color::srgb(0.20, 0.42, 0.36),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.pause.quicksave"),
                    SidebarAction::QuickSaveMatch,
                    Color::srgb(0.20, 0.36, 0.42),
                    Val::Percent(100.0),
                );
                if !interaction.is_multiplayer {
                    spawn_styled_button(
                        col,
                        &ui_t(loaded, interaction, "ui.pause.quickload"),
                        SidebarAction::QuickLoadMatch,
                        Color::srgb(0.20, 0.32, 0.42),
                        Val::Percent(100.0),
                    );
                }
                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.pause.options"),
                    SidebarAction::NavigateMenu(MenuScreen::Settings),
                    Color::srgb(0.24, 0.28, 0.38),
                    Val::Percent(100.0),
                );
                if !interaction.is_multiplayer {
                    spawn_styled_button(
                        col,
                        &ui_t(loaded, interaction, "ui.pause.restart"),
                        SidebarAction::RestartMatch,
                        Color::srgb(0.42, 0.26, 0.16),
                        Val::Percent(100.0),
                    );
                }
                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.menu.return_main"),
                    SidebarAction::ReturnToMainMenu,
                    Color::srgb(0.44, 0.18, 0.20),
                    Val::Percent(100.0),
                );
                spawn_styled_button(
                    col,
                    &ui_t(loaded, interaction, "ui.menu.exit"),
                    SidebarAction::ExitApplication,
                    Color::srgb(0.36, 0.12, 0.14),
                    Val::Percent(100.0),
                );
            });
        });
}
