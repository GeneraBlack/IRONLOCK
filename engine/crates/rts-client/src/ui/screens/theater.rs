use super::{spawn_body_text, spawn_screen_card};
use crate::input::ClientInteractionState;
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::spawn_styled_button;
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

pub fn spawn_theater_screen(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    _interaction: &ClientInteractionState,
    video_res: Option<&VideoPlaybackResource>,
    save_files: &[PathBuf],
    replay_files: &[PathBuf],
) {
    // Column 1: Saved Matches
    spawn_screen_card(
        parent,
        32.0,
        "1. SAVED TACTICAL MATCHES",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            spawn_styled_button(
                col,
                "QUICK-LOAD LAST SAVE [F8]",
                SidebarAction::QuickLoadMatch,
                Color::srgb(0.18, 0.42, 0.28),
                Val::Percent(100.0),
            );
            let max_scroll = (save_files.len() as f32 * 34.0 - 320.0).max(0.0);
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    overflow: Overflow::clip_y(),
                    ..default()
                },
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
                        spawn_body_text(
                            list,
                            "No saved matches found in saves/*.json.",
                            Color::srgb(0.68, 0.78, 0.88),
                        );
                    }
                    for path in save_files {
                        let fname = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("save.json");
                        spawn_styled_button(
                            list,
                            &format!("Load: {fname}"),
                            SidebarAction::LoadSaveFile(path.clone()),
                            Color::srgb(0.18, 0.28, 0.40),
                            Val::Percent(100.0),
                        );
                    }
                });
            });
        },
    );

    // Column 2: Deterministic Replays
    spawn_screen_card(
        parent,
        33.0,
        "2. DETERMINISTIC REPLAY ARCHIVE",
        Color::srgb(0.65, 0.88, 1.0),
        |col| {
            let max_scroll = (replay_files.len() as f32 * 34.0 - 360.0).max(0.0);
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    overflow: Overflow::clip_y(),
                    ..default()
                },
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
                    if replay_files.is_empty() {
                        spawn_body_text(
                            list,
                            "No deterministic replays recorded yet (saves/*.replay.json).",
                            Color::srgb(0.68, 0.78, 0.88),
                        );
                    }
                    for path in replay_files {
                        let fname = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("match.replay.json");
                        spawn_styled_button(
                            list,
                            &format!("Watch: {fname}"),
                            SidebarAction::PlayReplayFile(path.clone()),
                            Color::srgb(0.24, 0.24, 0.44),
                            Val::Percent(100.0),
                        );
                    }
                });
            });
        },
    );

    // Column 3: Native In-Engine FMV Player & Music Jukebox (zero external processes!)
    spawn_screen_card(
        parent,
        35.0,
        "3. IN-ENGINE FMV & JUKEBOX",
        Color::srgb(0.58, 0.95, 0.74),
        |col| {
            if let Some(vid) = video_res {
                col.spawn((
                    ImageNode::new(vid.frame_image.clone()),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(130.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(Color::srgb(0.36, 0.54, 0.78)),
                ));
            }
            let mut fmv_clips: Vec<(String, String)> = Vec::new();
            for camp in loaded.campaigns.values() {
                for m in &camp.missions {
                    if let Some(v) = &m.intro_video {
                        if !fmv_clips.iter().any(|(_, p)| p == v) {
                            fmv_clips.push((m.display_name.clone(), v.clone()));
                        }
                    }
                }
            }
            for (title, rel) in fmv_clips.into_iter().take(4) {
                spawn_styled_button(
                    col,
                    &format!("FMV: {title}"),
                    SidebarAction::PlayModVideo(rel),
                    Color::srgb(0.36, 0.22, 0.16),
                    Val::Percent(100.0),
                );
            }

            spawn_styled_button(
                col,
                "STOP JUKEBOX MUSIC",
                SidebarAction::StopModMusic,
                Color::srgb(0.42, 0.16, 0.18),
                Val::Percent(100.0),
            );
            for track in loaded.audio.music_tracks.iter().take(6) {
                spawn_styled_button(
                    col,
                    &format!("Music: {}", track.display_name),
                    SidebarAction::PlayModMusicTrack(track.id.clone()),
                    Color::srgb(0.18, 0.32, 0.46),
                    Val::Percent(100.0),
                );
            }
        },
    );
}
