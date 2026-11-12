use super::{spawn_body_text, spawn_screen_card};
use crate::audio::SoundSettingsResource;
use crate::input::{ClientInteractionState, MenuScreen};
use crate::ui::player_windows::options::spawn_options_window;
use crate::ui::studio_layout::{spawn_styled_button, UiFontResource};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;

pub fn spawn_settings_screen(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    sound_res: &SoundSettingsResource,
) {
    spawn_screen_card(
        parent,
        62.0,
        "CLIENT GRAPHICS, 5-BUS AUDIO & CONTROL SETTINGS",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            spawn_options_window(
                col,
                ui_font,
                loaded,
                interaction,
                sound_res,
                Color::srgb(0.32, 0.68, 0.96),
            );
            if interaction.settings_opened_from_pause {
                spawn_styled_button(
                    col,
                    "RETURN TO PAUSE MENU [ESC]",
                    SidebarAction::NavigateMenu(MenuScreen::PauseMenu),
                    Color::srgb(0.20, 0.36, 0.48),
                    Val::Percent(100.0),
                );
            }
        },
    );

    spawn_screen_card(
        parent,
        38.0,
        "TACTICAL COMMAND REFERENCE",
        Color::srgb(0.65, 0.88, 1.0),
        |col| {
            spawn_body_text(
                col,
                "BATTLEFIELD & SUB-WINDOWS:\n\
                 • [LMB / Drag]: Select Units & Structures\n\
                 • [RMB / Hold-Drag]: Move / Attack / Custom Line Formation\n\
                 • [Shift + Order]: Universal Command Queueing\n\
                 • [Arrow Keys / WASD / MMB]: Pan Camera (Alt+MMB Orbit)\n\
                 • [D]: Diplomacy, Alliance & Tribute Window\n\
                 • [Y]: Interactive Faction Tech-Tree ([T]: Cycle Unit Stance)\n\
                 • [Tab]: Mission Objectives & EVA Logbook\n\
                 • [F5 / F8]: Quick-Save / Quick-Load (.rtssav)\n\
                 • [ESC]: Close Topmost Window -> Cancel -> Pause Menu\n\n\
                 FORMATION & UNIT ORDERS:\n\
                 • [Ctrl + 0..9]: Assign Control Group\n\
                 • [J] / [Ctrl+J]: Cycle Formation / Convoy Speed\n\
                 • [K]: Tactical Reverse Move  |  [N]: Upgrade Structure\n\
                 • [U]: Unload Transport / Garrison\n\n\
                 CREATOR SUITE:\n\
                 • [F12]: Toggle IRONLOCK Creator Workbench IDE\n\
                 • [Ctrl+F12]: Transactional Live Mod Hot-Reload",
                Color::srgb(0.84, 0.91, 0.98),
            );
        },
    );
}
