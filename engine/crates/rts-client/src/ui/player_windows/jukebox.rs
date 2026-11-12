use crate::audio::{JukeboxState, JukeboxTrackEntry, MusicMood};
use crate::input::ClientInteractionState;
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use bevy::prelude::*;

const TRACKS_PER_PAGE: usize = 8;

fn mood_badge(mood: MusicMood) -> (&'static str, Color) {
    match mood {
        MusicMood::Menu => ("MENU", Color::srgb(0.64, 0.78, 0.98)),
        MusicMood::Peace => ("PEACE", Color::srgb(0.46, 0.90, 0.68)),
        MusicMood::Combat => ("COMBAT", Color::srgb(0.98, 0.48, 0.42)),
        MusicMood::Score => ("SCORE", Color::srgb(0.98, 0.88, 0.38)),
    }
}

pub fn spawn_jukebox_window(
    body: &mut ChildBuilder,
    interaction: &ClientInteractionState,
    jukebox_opt: Option<&JukeboxState>,
    border_col: Color,
) {
    let lang = interaction.language.as_str();
    let default_jb = JukeboxState::default();
    let jb = jukebox_opt.unwrap_or(&default_jb);

    // 1. Now Playing Banner
    let now_playing_str = if let Some(cur) = jb.current_track() {
        let state_lbl = if jb.is_playing {
            tr(lang, "jukebox_playing")
        } else {
            tr(lang, "jukebox_paused")
        };
        let facs = if cur.factions.is_empty() {
            "ALL".to_string()
        } else {
            cur.factions.join("/").to_uppercase()
        };
        format!("[{state_lbl}] {}  —  [{:?} | {facs}]", cur.title, cur.mood)
    } else {
        tr(lang, "jukebox_no_track").to_string()
    };

    body.spawn((
        Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.5)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.12, 0.20, 0.95)),
        BorderColor(border_col),
    ))
    .with_children(|hdr| {
        hdr.spawn((
            Text::new(sanitize_ui_text(&now_playing_str)),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::srgb(0.98, 0.92, 0.54)),
        ));
    });

    // 2. Transport & Mode Controls Row
    body.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(6.0),
        row_gap: Val::Px(5.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        spawn_styled_button(
            row,
            "|<< PREV",
            SidebarAction::JukeboxPrevTrack,
            Color::srgb(0.18, 0.28, 0.42),
            Val::Auto,
        );
        spawn_styled_button(
            row,
            if jb.is_playing { "PAUSE ||" } else { "PLAY >" },
            SidebarAction::JukeboxTogglePause,
            if jb.is_playing {
                Color::srgb(0.20, 0.48, 0.32)
            } else {
                Color::srgb(0.44, 0.30, 0.14)
            },
            Val::Auto,
        );
        spawn_styled_button(
            row,
            "NEXT >>|",
            SidebarAction::JukeboxNextTrack,
            Color::srgb(0.18, 0.28, 0.42),
            Val::Auto,
        );
        spawn_styled_button(
            row,
            if jb.shuffle {
                "SHUFFLE: ON"
            } else {
                "SHUFFLE: OFF"
            },
            SidebarAction::JukeboxToggleShuffle,
            if jb.shuffle {
                Color::srgb(0.24, 0.46, 0.36)
            } else {
                Color::srgb(0.16, 0.22, 0.32)
            },
            Val::Auto,
        );
        spawn_styled_button(
            row,
            if jb.repeat_one {
                "REPEAT: ONE"
            } else {
                "REPEAT: ALL"
            },
            SidebarAction::JukeboxToggleRepeat,
            if jb.repeat_one {
                Color::srgb(0.36, 0.38, 0.16)
            } else {
                Color::srgb(0.16, 0.22, 0.32)
            },
            Val::Auto,
        );
    });

    // 3. Dynamic Combat Crossfade & Faction Filter Row
    let fac_label = match jb.faction_filter.as_deref() {
        Some(f) if !f.is_empty() => format!("FILTER: {}", f.to_uppercase()),
        _ => "FILTER: ALL FACTIONS".to_string(),
    };
    body.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|row| {
        spawn_styled_button(
            row,
            if jb.dynamic_combat_mode {
                "DYNAMIC COMBAT CROSSFADE: ON"
            } else {
                "DYNAMIC COMBAT CROSSFADE: OFF"
            },
            SidebarAction::JukeboxToggleDynamicCombat,
            if jb.dynamic_combat_mode {
                Color::srgb(0.42, 0.22, 0.16)
            } else {
                Color::srgb(0.18, 0.22, 0.30)
            },
            Val::Auto,
        );
        spawn_styled_button(
            row,
            &fac_label,
            SidebarAction::JukeboxCycleFactionFilter,
            Color::srgb(0.22, 0.32, 0.48),
            Val::Auto,
        );
    });

    // 4. Filtered Track List (Paginated)
    let filtered: Vec<(usize, &JukeboxTrackEntry)> = jb
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| JukeboxState::track_matches_faction(t, jb.faction_filter.as_deref()))
        .collect();

    let total_pages = filtered.len().div_ceil(TRACKS_PER_PAGE).max(1);
    let page = interaction.jukebox_page.min(total_pages - 1);
    let start = page * TRACKS_PER_PAGE;
    let end = (start + TRACKS_PER_PAGE).min(filtered.len());

    body.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        ..default()
    })
    .with_children(|list| {
        if filtered.is_empty() {
            list.spawn((
                Text::new("No soundtrack tracks matched the active filter."),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.78, 0.88)),
            ));
        } else {
            for &(track_idx, track) in &filtered[start..end] {
                let is_cur = jb.current_index == Some(track_idx);
                let (mood_lbl, mood_col) = mood_badge(track.mood);
                let facs = if track.factions.is_empty() {
                    "ALL".to_string()
                } else {
                    track.factions.join("/").to_uppercase()
                };

                list.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(if is_cur {
                        Color::srgba(0.14, 0.30, 0.24, 0.94)
                    } else {
                        Color::srgba(0.06, 0.09, 0.15, 0.88)
                    }),
                    BorderColor(if is_cur {
                        Color::srgb(0.42, 0.94, 0.62)
                    } else {
                        Color::srgba(0.24, 0.36, 0.52, 0.65)
                    }),
                ))
                .with_children(|item| {
                    item.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|left| {
                        left.spawn((
                            Text::new(sanitize_ui_text(&format!(
                                "{:02}. {}{}",
                                track_idx + 1,
                                if is_cur { ">> " } else { "" },
                                track.title
                            ))),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(if is_cur {
                                Color::srgb(0.98, 0.95, 0.56)
                            } else {
                                Color::srgb(0.90, 0.95, 1.0)
                            }),
                        ));
                        left.spawn((
                            Text::new(format!("[{mood_lbl} | {facs}]")),
                            TextFont {
                                font_size: 9.5,
                                ..default()
                            },
                            TextColor(mood_col),
                        ));
                    });

                    spawn_styled_button(
                        item,
                        if is_cur && jb.is_playing {
                            "PLAYING"
                        } else {
                            "PLAY"
                        },
                        SidebarAction::JukeboxPlayTrack(track_idx),
                        if is_cur {
                            Color::srgb(0.22, 0.50, 0.34)
                        } else {
                            Color::srgb(0.18, 0.30, 0.46)
                        },
                        Val::Px(74.0),
                    );
                });
            }
        }
    });

    // 5. Pagination Footer
    body.spawn(Node {
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
            SidebarAction::JukeboxCyclePage(-1),
            Color::srgb(0.16, 0.24, 0.36),
            Val::Auto,
        );
        nav.spawn((
            Text::new(format!(
                "Tracks: {}  |  Page {}/{}",
                filtered.len(),
                page + 1,
                total_pages
            )),
            TextFont {
                font_size: 10.5,
                ..default()
            },
            TextColor(Color::srgb(0.76, 0.84, 0.94)),
        ));
        spawn_styled_button(
            nav,
            "NEXT PAGE >",
            SidebarAction::JukeboxCyclePage(1),
            Color::srgb(0.16, 0.24, 0.36),
            Val::Auto,
        );
    });
}
