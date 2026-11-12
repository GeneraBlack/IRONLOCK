use crate::input::ClientInteractionState;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_text_input, UiScrollContainer, UiScrollContent, UiTextFieldTarget};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MultiCampaignSaveStore, SaveGameEnvelope};
use std::path::{Path, PathBuf};

pub fn spawn_save_load_window(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    save_files: &[PathBuf],
) {
    spawn_text_input(
        parent,
        ui_font,
        "Save Slot Name (saved as saves/<name>.rtssav):",
        UiTextFieldTarget::SaveSlotName,
        &interaction.save_slot_name_input,
        true,
        false,
    );

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            spawn_styled_button(
                row,
                "SAVE NAMED SLOT",
                SidebarAction::SaveNamedSlot,
                Color::srgb(0.18, 0.46, 0.28),
                Val::Percent(if interaction.is_multiplayer {
                    55.0
                } else {
                    40.0
                }),
            );
            spawn_styled_button(
                row,
                "QUICKSAVE [F5]",
                SidebarAction::QuickSaveMatch,
                Color::srgb(0.20, 0.38, 0.48),
                Val::Percent(if interaction.is_multiplayer {
                    45.0
                } else {
                    30.0
                }),
            );
            if !interaction.is_multiplayer {
                spawn_styled_button(
                    row,
                    "QUICKLOAD [F8]",
                    SidebarAction::QuickLoadMatch,
                    Color::srgb(0.24, 0.32, 0.46),
                    Val::Percent(30.0),
                );
            }
        });

    if interaction.is_multiplayer {
        parent.spawn((
            Text::new(
                "Multiplayer .rtssav Save Active: Resume saved MP matches with all peers from the Host Staging Room!",
            ),
            TextFont {
                font_size: 10.5,
                ..default()
            },
            TextColor(Color::srgb(0.68, 0.92, 0.78)),
        ));
    }

    // Campaign Multi-Mod Progress Summary (`saves/campaign_progress.json`)
    let store = MultiCampaignSaveStore::load_or_default(Path::new("saves/campaign_progress.json"));
    let camp_count: usize = store
        .profiles
        .iter()
        .filter(|(k, _)| k.starts_with(&format!("{}::", loaded.manifest.id)))
        .map(|(_, p)| p.completed_missions.len())
        .sum();
    parent.spawn((
        Text::new(sanitize_ui_text(&format!(
            "Campaign Store ({}): {} completed operation(s) across {} profile(s)  |  Saved Slots: {}",
            loaded.manifest.id,
            camp_count,
            store.profiles.len(),
            save_files.len()
        ))),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgb(0.72, 0.88, 0.96)),
    ));

    let max_scroll = (save_files.len() as f32 * 46.0 - 220.0).max(0.0);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(240.0),
                overflow: Overflow::clip_y(),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.90)),
            BorderColor(Color::srgba(0.36, 0.54, 0.78, 0.75)),
            UiScrollContainer::new(max_scroll),
        ))
        .with_children(|vp| {
            vp.spawn((
                UiScrollContent,
                Node {
                    width: Val::Percent(100.0),
                    position_type: PositionType::Relative,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|list| {
                if save_files.is_empty() {
                    list.spawn((
                        Text::new(
                            "No save slots in saves/ yet. Enter a name above and click SAVE NAMED SLOT or press [F5].",
                        ),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.70, 0.78, 0.88)),
                    ));
                }
                let mod_hash = loaded.content_hash();
                for path in save_files {
                    let stem = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("slot.rtssav");
                    let (label, compatible) = match SaveGameEnvelope::read_header_only(path) {
                        Ok(hdr) => {
                            let compat = hdr.is_mod_compatible(&loaded.manifest.id, mod_hash);
                            let mins = hdr.play_seconds / 60;
                            let secs = hdr.play_seconds % 60;
                            let tag = if compat { "OK" } else { "MOD MISMATCH" };
                            (
                                format!(
                                    "[{}] {} — {} ({:02}:{:02}, ${})",
                                    tag, hdr.save_title, hdr.map_name, mins, secs, hdr.player_credits
                                ),
                                compat,
                            )
                        }
                        Err(_) => (format!("[JSON] {stem}"), true),
                    };

                    list.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_styled_button(
                            row,
                            &label,
                            SidebarAction::LoadSaveSlot(path.clone()),
                            if compatible {
                                Color::srgb(0.16, 0.32, 0.46)
                            } else {
                                Color::srgb(0.42, 0.28, 0.14)
                            },
                            Val::Percent(82.0),
                        );
                        spawn_styled_button(
                            row,
                            "DEL",
                            SidebarAction::DeleteSaveSlot(path.clone()),
                            Color::srgb(0.46, 0.16, 0.18),
                            Val::Percent(18.0),
                        );
                    });
                }
            });
        });
}
