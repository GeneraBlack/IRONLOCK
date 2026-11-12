use super::{spawn_body_text, spawn_screen_card};
use crate::input::{ClientInteractionState, MenuScreen};
use crate::ui::locale::ui_t;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_text_input, UiTextFieldTarget};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_network::lobby_registry::{WorkshopItemKind, WorkshopSortOrder};
use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

const WORKSHOP_ITEMS_PER_PAGE: usize = 5;

fn sort_label(sort: WorkshopSortOrder) -> &'static str {
    match sort {
        WorkshopSortOrder::Downloads => "TOP DOWNLOADS",
        WorkshopSortOrder::Newest => "NEWEST",
        WorkshopSortOrder::Updated => "RECENTLY UPDATED",
        WorkshopSortOrder::Title => "TITLE A-Z",
    }
}

pub fn spawn_workshop_screen(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    discovered_mods: &[PathBuf],
) {
    // Column 1 (34%): Search, Kind Filter, Sort & Installed Mods
    spawn_screen_card(
        parent,
        34.0,
        &ui_t(loaded, interaction, "ui.workshop.title"),
        Color::srgb(0.58, 0.90, 1.0),
        |col| {
            spawn_body_text(
                col,
                format!("Registry Endpoint: {}", interaction.lobby_server_url),
                Color::srgb(0.74, 0.88, 0.98),
            );

            spawn_text_input(
                col,
                ui_font,
                "Search Mods & Custom Maps (Title / Author / Tag):",
                UiTextFieldTarget::WorkshopSearch,
                &interaction.workshop_search_query,
                true,
                interaction.workshop_search_editing,
            );

            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for (lbl, filt) in [
                    ("ALL", None),
                    ("MODS (.rtsmod)", Some(WorkshopItemKind::Mod)),
                    ("MAPS (.toml)", Some(WorkshopItemKind::Map)),
                ] {
                    let active = interaction.workshop_kind_filter == filt;
                    spawn_styled_button(
                        row,
                        lbl,
                        SidebarAction::SetWorkshopKindFilter(filt),
                        if active {
                            Color::srgb(0.22, 0.48, 0.34)
                        } else {
                            Color::srgb(0.16, 0.24, 0.36)
                        },
                        Val::Percent(33.0),
                    );
                }
            });

            spawn_styled_button(
                col,
                &format!(
                    "SORT BY: {} (CLICK TO CYCLE)",
                    sort_label(interaction.workshop_sort)
                ),
                SidebarAction::CycleWorkshopSort,
                Color::srgb(0.22, 0.30, 0.44),
                Val::Percent(100.0),
            );

            spawn_styled_button(
                col,
                "QUERY / REFRESH WORKSHOP CATALOG",
                SidebarAction::RefreshWorkshopItems,
                Color::srgb(0.18, 0.44, 0.56),
                Val::Percent(100.0),
            );

            spawn_body_text(
                col,
                &interaction.workshop_status_text,
                Color::srgb(0.62, 0.96, 0.76),
            );

            spawn_body_text(
                col,
                format!("Installed Local Mods ({}):", discovered_mods.len()),
                Color::srgb(0.96, 0.88, 0.50),
            );
            for mpath in discovered_mods.iter().take(5) {
                let name = mpath.file_name().and_then(|s| s.to_str()).unwrap_or("mod");
                let active = name == loaded.manifest.id
                    || name.trim_end_matches(".rtsmod") == loaded.manifest.id;
                spawn_styled_button(
                    col,
                    &format!(
                        "{}{}",
                        if active { "[ACTIVE] " } else { "ACTIVATE: " },
                        name
                    ),
                    SidebarAction::LoadModFromPath(mpath.clone()),
                    if active {
                        Color::srgb(0.22, 0.46, 0.30)
                    } else {
                        Color::srgb(0.16, 0.22, 0.32)
                    },
                    Val::Percent(100.0),
                );
            }

            spawn_styled_button(
                col,
                "<< BACK TO MAIN MENU",
                SidebarAction::NavigateMenu(MenuScreen::Root),
                Color::srgb(0.38, 0.18, 0.20),
                Val::Percent(100.0),
            );
        },
    );

    // Column 2 (66%): Workshop Package Cards + One-Click SHA-256 Verified Download
    spawn_screen_card(
        parent,
        66.0,
        "COMMUNITY .RTSMOD PACKAGES & TACTICAL MAPS",
        Color::srgb(0.98, 0.86, 0.42),
        |col| {
            if let Some((ref missing_mod_id, ref join_addr)) =
                interaction.pending_missing_mod_prompt
            {
                col.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.34, 0.18, 0.08, 0.94)),
                    BorderColor(Color::srgb(0.98, 0.76, 0.30)),
                ))
                .with_children(|banner| {
                    banner.spawn((
                        Text::new(sanitize_ui_text(&format!(
                            "Lobby requires mod '{missing_mod_id}'. Download & auto-join {join_addr}?"
                        ))),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.92, 0.64)),
                    ));
                    spawn_styled_button(
                        banner,
                        "DOWNLOAD & JOIN",
                        SidebarAction::DownloadWorkshopItem {
                            item_id: missing_mod_id.clone(),
                            sha256: None,
                            auto_join_addr: Some(join_addr.clone()),
                        },
                        Color::srgb(0.20, 0.50, 0.30),
                        Val::Auto,
                    );
                    spawn_styled_button(
                        banner,
                        "DISMISS",
                        SidebarAction::DismissMissingModPrompt,
                        Color::srgb(0.40, 0.18, 0.20),
                        Val::Auto,
                    );
                });
            }

            let total_pages = interaction
                .workshop_items
                .len()
                .div_ceil(WORKSHOP_ITEMS_PER_PAGE)
                .max(1);
            let page = interaction.workshop_page.min(total_pages - 1);
            let start = page * WORKSHOP_ITEMS_PER_PAGE;
            let end = (start + WORKSHOP_ITEMS_PER_PAGE).min(interaction.workshop_items.len());

            if interaction.workshop_items.is_empty() {
                spawn_body_text(
                    col,
                    "No Workshop items cached yet. Click 'QUERY / REFRESH WORKSHOP CATALOG' to browse published .rtsmod archives and custom maps, or publish your own mod from the Creator Workbench Export Wizard!",
                    Color::srgb(0.76, 0.84, 0.94),
                );
            } else {
                for item in &interaction.workshop_items[start..end] {
                    let kind_tag = match item.kind {
                        WorkshopItemKind::Mod => "MOD",
                        WorkshopItemKind::Map => "MAP",
                    };
                    let kb = (item.size_bytes / 1024).max(1);
                    col.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(10.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.10, 0.17, 0.92)),
                        BorderColor(Color::srgb(0.30, 0.48, 0.72)),
                    ))
                    .with_children(|card| {
                        card.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(3.0),
                            flex_grow: 1.0,
                            ..default()
                        })
                        .with_children(|info| {
                            info.spawn((
                                Text::new(sanitize_ui_text(&format!(
                                    "[{kind_tag}] {} v{}  —  by {}  ({} DLs | {} KB)",
                                    item.title, item.version, item.author_name, item.downloads, kb
                                ))),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.92, 0.56)),
                            ));
                            if !item.description.trim().is_empty() {
                                info.spawn((
                                    Text::new(sanitize_ui_text(&item.description)),
                                    TextFont {
                                        font_size: 10.5,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.82, 0.90, 0.98)),
                                ));
                            }
                        });

                        spawn_styled_button(
                            card,
                            "DOWNLOAD",
                            SidebarAction::DownloadWorkshopItem {
                                item_id: item.id.clone(),
                                sha256: Some(item.sha256.clone()),
                                auto_join_addr: None,
                            },
                            Color::srgb(0.20, 0.46, 0.32),
                            Val::Px(116.0),
                        );
                    });
                }
            }

            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|nav| {
                spawn_styled_button(
                    nav,
                    "< PREV PAGE",
                    SidebarAction::CycleWorkshopPage(-1),
                    Color::srgb(0.16, 0.24, 0.36),
                    Val::Px(120.0),
                );
                spawn_body_text(
                    nav,
                    format!(
                        "Items: {}  |  Page {}/{}",
                        interaction.workshop_items.len(),
                        page + 1,
                        total_pages
                    ),
                    Color::srgb(0.78, 0.86, 0.96),
                );
                spawn_styled_button(
                    nav,
                    "NEXT PAGE >",
                    SidebarAction::CycleWorkshopPage(1),
                    Color::srgb(0.16, 0.24, 0.36),
                    Val::Px(120.0),
                );
            });
        },
    );
}

pub(super) fn spawn_staging_spectator_and_resume_controls(
    col: &mut ChildBuilder,
    room: &rts_network::lobby_room::LobbyRoomState,
    is_host: bool,
    save_files: &[std::path::PathBuf],
    interaction: &ClientInteractionState,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(4.0),
        ..default()
    })
    .with_children(|srow| {
        spawn_styled_button(
            srow,
            if room.rules.allow_spectators {
                "SPECTATE: ON"
            } else {
                "SPECTATE: OFF"
            },
            SidebarAction::StagingToggleAllowSpectators,
            if room.rules.allow_spectators {
                Color::srgb(0.20, 0.44, 0.34)
            } else {
                Color::srgb(0.24, 0.24, 0.30)
            },
            Val::Percent(52.0),
        );
        spawn_styled_button(
            srow,
            &format!("DELAY: {}s", room.rules.spectator_delay_secs),
            SidebarAction::StagingCycleSpectatorDelay,
            Color::srgb(0.22, 0.30, 0.46),
            Val::Percent(48.0),
        );
    });

    if let Some(ref sav_name) = room.rules.resume_save_name {
        spawn_body_text(
            col,
            format!(
                "Resuming Saved MP Match: {} (Tick #{})",
                sav_name,
                room.rules.resume_tick.unwrap_or(0)
            ),
            Color::srgb(0.98, 0.88, 0.44),
        );
    }
    if is_host {
        let rtssav_files: Vec<&std::path::PathBuf> = save_files
            .iter()
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("rtssav"))
            .collect();
        if let Some(&first_sav) =
            rtssav_files.get(interaction.mp_resume_save_page % rtssav_files.len().max(1))
        {
            let fname = first_sav
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("mp.rtssav");
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|sav_row| {
                spawn_styled_button(
                    sav_row,
                    &format!("RESUME .RTSSAV: {fname}"),
                    SidebarAction::StagingSelectResumeSave(first_sav.clone()),
                    Color::srgb(0.24, 0.42, 0.32),
                    Val::Percent(72.0),
                );
                let (lbl, act) = if room.rules.resume_save_hash.is_some() {
                    ("CLEAR", SidebarAction::StagingClearResumeSave)
                } else {
                    ("NEXT", SidebarAction::StagingCycleResumeSavePage(1))
                };
                spawn_styled_button(
                    sav_row,
                    lbl,
                    act,
                    Color::srgb(0.32, 0.24, 0.30),
                    Val::Percent(28.0),
                );
            });
        }
    }
}
