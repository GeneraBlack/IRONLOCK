use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::rendering::PreviewStageCamera;
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::schema::{CameraCutsceneDef, CameraKeyframe};
use rts_simulation::world::SimEvaTransmission;
use std::collections::VecDeque;

#[derive(Resource, Debug, Clone, Default)]
pub struct CutsceneDirectorState {
    pub active_cutscene: Option<CameraCutsceneDef>,
    pub cutscene_elapsed_secs: f32,
    pub saved_camera_pose: Option<(Vec3, f32, f32, f32)>,
    pub queued_cutscenes: VecDeque<CameraCutsceneDef>,
    pub seen_rich_eva_count: usize,
    pub queued_transmissions: VecDeque<SimEvaTransmission>,
    pub active_transmission: Option<SimEvaTransmission>,
    pub transmission_remaining_secs: f32,
}

#[derive(Component)]
pub struct CutsceneTopLetterboxNode;

#[derive(Component)]
pub struct CutsceneBottomLetterboxNode;

#[derive(Component)]
pub struct CutsceneSkipButtonContainer;

#[derive(Component)]
pub struct RichTransmissionCardNode;

#[derive(Component)]
pub struct RichTransmissionPortraitNode;

#[derive(Component)]
pub struct RichTransmissionSpeakerText;

#[derive(Component)]
pub struct RichTransmissionBodyText;

/// Smoothly interpolates `CameraKeyframe` tracks (`focus`, `distance`, `pitch_deg`, `yaw_deg`)
/// using cubic smoothstep (`3t^2 - 2t^3`) and shortest-arc yaw interpolation.
pub fn sample_cutscene_pose(keyframes: &[CameraKeyframe], time_secs: f32) -> (Vec3, f32, f32, f32) {
    if keyframes.is_empty() {
        return (Vec3::ZERO, 36.0, 56.0_f32.to_radians(), 0.0);
    }
    if keyframes.len() == 1 || time_secs <= keyframes[0].time_secs {
        let kf = &keyframes[0];
        return (
            Vec3::from_array(kf.focus),
            kf.distance,
            kf.pitch_deg.to_radians(),
            kf.yaw_deg.to_radians(),
        );
    }
    let last = keyframes.last().unwrap();
    if time_secs >= last.time_secs {
        return (
            Vec3::from_array(last.focus),
            last.distance,
            last.pitch_deg.to_radians(),
            last.yaw_deg.to_radians(),
        );
    }

    for pair in keyframes.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if time_secs >= a.time_secs && time_secs <= b.time_secs {
            let span = (b.time_secs - a.time_secs).max(0.0001);
            let raw_t = ((time_secs - a.time_secs) / span).clamp(0.0, 1.0);
            let s = raw_t * raw_t * (3.0 - 2.0 * raw_t);

            let focus = Vec3::from_array(a.focus).lerp(Vec3::from_array(b.focus), s);
            let dist = a.distance + (b.distance - a.distance) * s;
            let pitch = a.pitch_deg.to_radians()
                + (b.pitch_deg.to_radians() - a.pitch_deg.to_radians()) * s;

            let yaw_a = a.yaw_deg.to_radians();
            let yaw_b = b.yaw_deg.to_radians();
            let mut delta_yaw = (yaw_b - yaw_a) % std::f32::consts::TAU;
            if delta_yaw > std::f32::consts::PI {
                delta_yaw -= std::f32::consts::TAU;
            } else if delta_yaw < -std::f32::consts::PI {
                delta_yaw += std::f32::consts::TAU;
            }
            let yaw = yaw_a + delta_yaw * s;
            return (focus, dist, pitch, yaw);
        }
    }

    (
        Vec3::from_array(last.focus),
        last.distance,
        last.pitch_deg.to_radians(),
        last.yaw_deg.to_radians(),
    )
}

pub fn cutscene_total_duration(cutscene: &CameraCutsceneDef) -> f32 {
    cutscene
        .keyframes
        .iter()
        .map(|kf| kf.time_secs)
        .fold(0.0_f32, f32::max)
}

pub fn spawn_cutscene_overlay(commands: &mut Commands) {
    commands.spawn((
        CutsceneTopLetterboxNode,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            height: Val::Px(68.0),
            display: Display::None,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
            border: UiRect::bottom(Val::Px(1.5)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.01, 0.02, 0.96)),
        BorderColor(Color::srgb(0.45, 0.62, 0.86)),
        ZIndex(60),
    ));

    commands
        .spawn((
            CutsceneBottomLetterboxNode,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Px(68.0),
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                border: UiRect::top(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.01, 0.02, 0.96)),
            BorderColor(Color::srgb(0.45, 0.62, 0.86)),
            ZIndex(60),
        ))
        .with_children(|bot| {
            bot.spawn((
                CutsceneSkipButtonContainer,
                Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
            ))
            .with_children(|btn_box| {
                spawn_styled_button(
                    btn_box,
                    "SKIP CUTSCENE [ESC]",
                    SidebarAction::SkipActiveCutscene,
                    Color::srgb(0.42, 0.18, 0.20),
                    Val::Auto,
                );
            });
        });

    commands
        .spawn((
            RichTransmissionCardNode,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(280.0),
                top: Val::Px(46.0),
                width: Val::Px(410.0),
                display: Display::None,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.07, 0.13, 0.95)),
            BorderColor(Color::srgb(0.42, 0.84, 1.0)),
            ZIndex(58),
            PickingBehavior::IGNORE,
        ))
        .with_children(|card| {
            card.spawn((
                RichTransmissionPortraitNode,
                ImageNode::default(),
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    border: UiRect::all(Val::Px(1.5)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.10, 0.18, 0.28)),
                BorderColor(Color::srgb(0.95, 0.82, 0.36)),
            ));

            card.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    RichTransmissionSpeakerText,
                    Text::new("EVA TRANSMISSION"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.88, 0.44)),
                ));
                col.spawn((
                    RichTransmissionBodyText,
                    Text::new(""),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.90, 0.96, 1.0)),
                ));
            });
        });
}

pub fn finish_or_skip_active_cutscene(
    director: &mut CutsceneDirectorState,
    rig: &mut RtsCameraRig,
    cam_tf: &mut Transform,
) {
    director.active_cutscene = None;
    director.cutscene_elapsed_secs = 0.0;
    rig.cutscene_override = false;
    if let Some((focus, dist, pitch, yaw)) = director.saved_camera_pose.take() {
        rig.focus = focus;
        rig.target_focus = focus;
        rig.distance = dist;
        rig.target_distance = dist;
        rig.pitch_radians = pitch;
        rig.target_pitch_radians = pitch;
        rig.yaw_radians = yaw;
        rig.target_yaw_radians = yaw;
        *cam_tf = rig.compute_transform();
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_cutscene_and_transmissions(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Option<Res<AssetServer>>,
    asset_root: Res<AssetRootResource>,
    mod_res: Res<ModResource>,
    mut sim_res: ResMut<SimWorldResource>,
    mut interaction: ResMut<ClientInteractionState>,
    mut ui_cache: ResMut<UiAssetCache>,
    mut director: ResMut<CutsceneDirectorState>,
    mut camera_q: Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
    (mut top_bar_q, mut bot_bar_q, mut skip_btn_q, mut card_q): (
        Query<
            &mut Node,
            (
                With<CutsceneTopLetterboxNode>,
                Without<CutsceneBottomLetterboxNode>,
                Without<CutsceneSkipButtonContainer>,
                Without<RichTransmissionCardNode>,
            ),
        >,
        Query<
            &mut Node,
            (
                With<CutsceneBottomLetterboxNode>,
                Without<CutsceneTopLetterboxNode>,
                Without<CutsceneSkipButtonContainer>,
                Without<RichTransmissionCardNode>,
            ),
        >,
        Query<
            &mut Node,
            (
                With<CutsceneSkipButtonContainer>,
                Without<CutsceneTopLetterboxNode>,
                Without<CutsceneBottomLetterboxNode>,
                Without<RichTransmissionCardNode>,
            ),
        >,
        Query<
            &mut Node,
            (
                With<RichTransmissionCardNode>,
                Without<CutsceneTopLetterboxNode>,
                Without<CutsceneBottomLetterboxNode>,
                Without<CutsceneSkipButtonContainer>,
            ),
        >,
    ),
    mut portrait_q: Query<&mut ImageNode, With<RichTransmissionPortraitNode>>,
    mut speaker_txt_q: Query<
        &mut Text,
        (
            With<RichTransmissionSpeakerText>,
            Without<RichTransmissionBodyText>,
        ),
    >,
    mut body_txt_q: Query<
        &mut Text,
        (
            With<RichTransmissionBodyText>,
            Without<RichTransmissionSpeakerText>,
        ),
    >,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let in_game = interaction.app_phase == AppPhase::InGame && !interaction.studio_open;

    // 1. Drain pending cutscenes emitted by simulation triggers
    if !sim_res.0.pending_cutscenes.is_empty() {
        let requested: Vec<String> = std::mem::take(&mut sim_res.0.pending_cutscenes);
        for cs_id in requested {
            if let Some(def) = mod_res
                .0
                .entry_map
                .cutscenes
                .iter()
                .find(|c| c.id == cs_id)
                .cloned()
            {
                director.queued_cutscenes.push_back(def);
            }
        }
    }

    if director.active_cutscene.is_none() && in_game && !interaction.menu_open {
        if let Some(next_cs) = director.queued_cutscenes.pop_front() {
            if let Ok((mut rig, _)) = camera_q.get_single_mut() {
                director.saved_camera_pose =
                    Some((rig.focus, rig.distance, rig.pitch_radians, rig.yaw_radians));
                rig.cutscene_override = true;
            }
            director.cutscene_elapsed_secs = 0.0;
            director.active_cutscene = Some(next_cs);
        }
    }

    // Drive active cutscene spline interpolation & `[Escape]` skip
    if let Some(cs) = director.active_cutscene.clone() {
        if !in_game {
            if let Ok((mut rig, mut cam_tf)) = camera_q.get_single_mut() {
                finish_or_skip_active_cutscene(&mut director, &mut rig, &mut cam_tf);
            }
        } else if cs.skippable && keys.just_pressed(KeyCode::Escape) {
            if let Ok((mut rig, mut cam_tf)) = camera_q.get_single_mut() {
                finish_or_skip_active_cutscene(&mut director, &mut rig, &mut cam_tf);
            }
            let skip_msg = tr(&interaction.language, "cutscene_skipped");
            interaction.push_eva_alert(skip_msg);
        } else {
            director.cutscene_elapsed_secs += dt;
            let total_dur = cutscene_total_duration(&cs);
            let (focus, dist, pitch, yaw) =
                sample_cutscene_pose(&cs.keyframes, director.cutscene_elapsed_secs);
            if let Ok((mut rig, mut cam_tf)) = camera_q.get_single_mut() {
                rig.cutscene_override = true;
                rig.focus = focus;
                rig.target_focus = focus;
                rig.distance = dist;
                rig.target_distance = dist;
                rig.pitch_radians = pitch;
                rig.target_pitch_radians = pitch;
                rig.yaw_radians = yaw;
                rig.target_yaw_radians = yaw;
                *cam_tf = rig.compute_transform();
                if director.cutscene_elapsed_secs >= total_dur {
                    finish_or_skip_active_cutscene(&mut director, &mut rig, &mut cam_tf);
                }
            }
        }
    }

    let show_letterbox = in_game
        && director
            .active_cutscene
            .as_ref()
            .is_some_and(|c| c.letterbox);
    let skippable = director
        .active_cutscene
        .as_ref()
        .is_some_and(|c| c.skippable);
    for mut node in &mut top_bar_q {
        node.display = if show_letterbox {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut bot_bar_q {
        node.display = if show_letterbox {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut skip_btn_q {
        node.display = if show_letterbox && skippable {
            Display::Flex
        } else {
            Display::None
        };
    }

    // 2. Non-destructively observe `SimWorld.rich_eva_transmissions` (so `audio.rs` also sees them!)
    let total_transmissions = sim_res.0.rich_eva_transmissions.len();
    if total_transmissions < director.seen_rich_eva_count {
        director.seen_rich_eva_count = 0;
        director.queued_transmissions.clear();
        director.active_transmission = None;
    }
    if total_transmissions > director.seen_rich_eva_count {
        for item in &sim_res.0.rich_eva_transmissions[director.seen_rich_eva_count..] {
            director.queued_transmissions.push_back(item.clone());
        }
        director.seen_rich_eva_count = total_transmissions;
    }

    if director.active_transmission.is_some() {
        director.transmission_remaining_secs -= dt;
        if director.transmission_remaining_secs <= 0.0 {
            director.active_transmission = None;
        }
    }

    if director.active_transmission.is_none() && in_game {
        if let Some(tx) = director.queued_transmissions.pop_front() {
            let localized_text = mod_res.0.t(&interaction.language, &tx.text);
            let speaker_label = if tx.speaker.trim().is_empty() {
                "COMMAND EVA".to_string()
            } else {
                mod_res.0.t(&interaction.language, &tx.speaker).to_string()
            };
            interaction.push_eva_alert(format!("{speaker_label}: {localized_text}"));

            if let Ok(mut spk) = speaker_txt_q.get_single_mut() {
                **spk = sanitize_ui_text(&speaker_label);
            }
            if let Ok(mut body) = body_txt_q.get_single_mut() {
                **body = sanitize_ui_text(localized_text);
            }
            if let Ok(mut img_node) = portrait_q.get_single_mut() {
                if let Some(ref port_rel) = tx.portrait {
                    if let Some(h) = ui_cache.load_qualified_mod_image(
                        &mod_res.0,
                        asset_server.as_deref(),
                        &asset_root.0,
                        port_rel,
                    ) {
                        img_node.image = h;
                    }
                }
            }

            director.transmission_remaining_secs = 6.5;
            director.active_transmission = Some(tx);
        }
    }

    let show_card = in_game && !interaction.menu_open && director.active_transmission.is_some();
    for mut node in &mut card_q {
        node.display = if show_card {
            Display::Flex
        } else {
            Display::None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutscene_smoothstep_interpolates_pose_and_shortest_yaw() {
        let kfs = vec![
            CameraKeyframe {
                time_secs: 0.0,
                focus: [10.0, 0.0, 10.0],
                distance: 40.0,
                pitch_deg: 60.0,
                yaw_deg: 350.0,
            },
            CameraKeyframe {
                time_secs: 4.0,
                focus: [30.0, 2.0, 50.0],
                distance: 24.0,
                pitch_deg: 40.0,
                yaw_deg: 10.0,
            },
        ];
        let (mid_focus, mid_dist, mid_pitch, mid_yaw) = sample_cutscene_pose(&kfs, 2.0);
        assert!((mid_focus.x - 20.0).abs() < 0.01);
        assert!((mid_focus.z - 30.0).abs() < 0.01);
        assert!((mid_dist - 32.0).abs() < 0.01);
        assert!((mid_pitch - 50.0_f32.to_radians()).abs() < 0.01);
        // Midpoint between 350 deg and +10 deg across shortest arc is 360 deg (0 rad mod TAU)
        let norm_yaw = mid_yaw.rem_euclid(std::f32::consts::TAU);
        assert!(norm_yaw < 0.02 || (std::f32::consts::TAU - norm_yaw) < 0.02);
    }
}
