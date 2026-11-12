use super::{BriefingVideoImageNode, BriefingVideoProgressFill, BriefingVideoSubtitleText};
use crate::input::ClientInteractionState;
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

pub fn spawn_briefing_window(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &ClientInteractionState,
    video_res: Option<&VideoPlaybackResource>,
) {
    let active_mission = interaction
        .active_campaign_id
        .as_ref()
        .and_then(|cid| loaded.campaigns.get(cid))
        .and_then(|camp| {
            interaction
                .active_mission_id
                .as_ref()
                .and_then(|mid| camp.missions.iter().find(|m| &m.id == mid))
                .or_else(|| camp.missions.first())
        });

    let title = active_mission
        .map(|m| format!("OPERATION: {}", m.display_name))
        .unwrap_or_else(|| format!("TACTICAL BRIEFING — {}", loaded.entry_map.name));
    let briefing_text = active_mission
        .map(|m| m.briefing.clone())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| world.mission_objective.clone())
        .unwrap_or_else(|| {
            "Establish forward base operations, secure primary resource veins, and neutralize all hostile command structures in the sector."
                .to_string()
        });

    parent.spawn((
        Text::new(sanitize_ui_text(&title)),
        TextFont {
            font_size: 13.5,
            ..default()
        },
        TextColor(Color::srgb(0.98, 0.88, 0.45)),
    ));

    // Native In-Engine FMV Viewport (`VideoPlaybackResource.frame_image`, zero external ffplay processes!)
    let initial_img = video_res.map(|v| v.frame_image.clone());
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(220.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.02, 0.03, 0.05)),
            BorderColor(Color::srgb(0.32, 0.48, 0.68)),
        ))
        .with_children(|vp| {
            if let Some(handle) = initial_img {
                vp.spawn((
                    BriefingVideoImageNode,
                    ImageNode::new(handle),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
            } else {
                vp.spawn((
                    BriefingVideoImageNode,
                    ImageNode::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ));
                vp.spawn((
                    Text::new("IN-ENGINE FMV TRANSMISSION CHANNEL READY"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.52, 0.72, 0.88)),
                ));
            }
        });

    // Progress Bar + Subtitle Strip
    let init_pct = video_res.map(|v| v.progress_01() * 100.0).unwrap_or(0.0);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(7.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.08, 0.12)),
            BorderColor(Color::srgb(0.28, 0.38, 0.52)),
        ))
        .with_children(|bar| {
            bar.spawn((
                BriefingVideoProgressFill,
                Node {
                    width: Val::Percent(init_pct),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.24, 0.86, 0.52)),
            ));
        });

    parent.spawn((
        BriefingVideoSubtitleText,
        Text::new("Ready for briefing playback."),
        TextFont {
            font_size: 11.5,
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.92, 1.0)),
    ));

    // Playback Transport Controls
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|ctrl| {
            spawn_styled_button(
                ctrl,
                "PLAY / PAUSE",
                SidebarAction::VideoTogglePause,
                Color::srgb(0.18, 0.42, 0.28),
                Val::Percent(33.0),
            );
            spawn_styled_button(
                ctrl,
                "SKIP / STOP",
                SidebarAction::VideoStopSkip,
                Color::srgb(0.46, 0.22, 0.18),
                Val::Percent(33.0),
            );
            let sample_clip = active_mission
                .and_then(|m| m.intro_video.clone().or_else(|| m.radar_video.clone()))
                .or_else(|| interaction.active_video_preview_path.clone())
                .unwrap_or_else(|| "assets/videos/briefing_preview.mp4".to_string());
            spawn_styled_button(
                ctrl,
                "REPLAY BRIEFING",
                SidebarAction::PlayModVideo(sample_clip),
                Color::srgb(0.20, 0.32, 0.50),
                Val::Percent(34.0),
            );
        });

    // Tactical Briefing Text Box
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.94)),
            BorderColor(Color::srgb(0.25, 0.36, 0.50)),
        ))
        .with_children(|box_node| {
            box_node.spawn((
                Text::new(sanitize_ui_text(&briefing_text)),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.92, 0.96)),
            ));
        });
}
