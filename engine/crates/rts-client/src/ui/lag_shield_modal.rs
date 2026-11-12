use crate::input::{AppPhase, ClientInteractionState, UiInputBlocker};
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::SidebarAction;
use crate::NetRuntimeResource;
use bevy::prelude::*;

#[derive(Component)]
pub struct LagShieldModalRoot;

#[derive(Component)]
pub struct LagShieldModalBody;

#[derive(Default)]
pub struct LagShieldSigCache(pub String);

pub fn spawn_lag_shield_modal(commands: &mut Commands) {
    commands
        .spawn((
            LagShieldModalRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.68)),
            ZIndex(80),
        ))
        .with_children(|backdrop| {
            backdrop.spawn((
                LagShieldModalBody,
                Node {
                    width: Val::Px(510.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.14, 0.97)),
                BorderColor(Color::srgb(0.98, 0.62, 0.24)),
            ));
        });
}

pub fn update_lag_shield_modal(
    mut commands: Commands,
    interaction: Res<ClientInteractionState>,
    net_res: Res<NetRuntimeResource>,
    mut ui_blocker: ResMut<UiInputBlocker>,
    mut sig_cache: Local<LagShieldSigCache>,
    mut root_q: Query<&mut Node, With<LagShieldModalRoot>>,
    body_q: Query<Entity, With<LagShieldModalBody>>,
) {
    let Ok(mut root_node) = root_q.get_single_mut() else {
        return;
    };

    let stall_opt = if interaction.app_phase == AppPhase::InGame && interaction.is_multiplayer {
        net_res
            .client
            .as_ref()
            .and_then(|mtx| mtx.lock().ok())
            .and_then(|cl| cl.peer_stall_status.clone())
    } else {
        None
    };

    let desync_text = if interaction.app_phase == AppPhase::InGame {
        interaction.desync_banner_text.clone()
    } else {
        None
    };

    if stall_opt.is_none() && desync_text.is_none() {
        root_node.display = Display::None;
        sig_cache.0.clear();
        return;
    }

    root_node.display = Display::Flex;
    ui_blocker.over_player_hud = true;

    let stall_sig = stall_opt
        .as_ref()
        .map(|s| {
            format!(
                "{:?}:{}:{}:{:?}:{:?}",
                s.stalled_players,
                s.grace_remaining_ms.div_ceil(1000),
                s.elapsed_ms / 1000,
                s.votes_drop,
                s.votes_ai
            )
        })
        .unwrap_or_default();
    let sig = format!(
        "st:{stall_sig}|ds:{desync_text:?}|fp:{:?}|lang:{}",
        interaction.desync_forensics_path, interaction.language
    );
    if sig_cache.0 == sig {
        return;
    }
    sig_cache.0 = sig;

    let Ok(body_ent) = body_q.get_single() else {
        return;
    };
    commands.entity(body_ent).despawn_descendants();

    let lang = interaction.language.clone();
    let local_pid = interaction.local_player_id;

    commands.entity(body_ent).with_children(|card| {
        if let Some(ref banner) = desync_text {
            card.spawn((
                Text::new(sanitize_ui_text(tr(&lang, "desync_alert_title"))),
                TextFont {
                    font_size: 14.5,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.42, 0.36)),
            ));
            card.spawn((
                Text::new(sanitize_ui_text(banner)),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.92, 0.88)),
            ));
            if let Some(ref path) = interaction.desync_forensics_path {
                card.spawn((
                    Text::new(sanitize_ui_text(&format!("Forensics Bundle: {path}"))),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.72, 0.86, 0.98)),
                ));
            }
            spawn_styled_button(
                card,
                tr(&lang, "btn_resync_host"),
                SidebarAction::ApplyAuthoritativeResync,
                Color::srgb(0.20, 0.50, 0.34),
                Val::Percent(100.0),
            );
        }

        if let Some(stall) = stall_opt {
            let grace_secs = stall.grace_remaining_ms.div_ceil(1000);
            let elapsed_secs = stall.elapsed_ms / 1000;
            card.spawn((
                Text::new(sanitize_ui_text(tr(&lang, "lag_shield_title"))),
                TextFont {
                    font_size: 14.5,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.78, 0.32)),
            ));

            let stalled_list = stall
                .stalled_players
                .iter()
                .map(|pid| format!("Player P{pid}"))
                .collect::<Vec<_>>()
                .join(", ");
            card.spawn((
                Text::new(sanitize_ui_text(&format!(
                    "Waiting for {stalled_list}...\nStalled: {elapsed_secs}s  |  Grace Countdown: {grace_secs}s",
                ))),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.90, 0.95, 1.0)),
            ));

            card.spawn((
                Text::new(format!(
                    "Votes to Drop: {}  |  Votes for AI Takeover: {}",
                    stall.votes_drop.len(),
                    stall.votes_ai.len()
                )),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.68, 0.84, 0.98)),
            ));

            for &target_pid in &stall.stalled_players {
                if target_pid == local_pid {
                    continue;
                }
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        &format!("VOTE DROP P{target_pid}"),
                        SidebarAction::CastLagStallVoteDrop(target_pid),
                        Color::srgb(0.48, 0.18, 0.20),
                        Val::Percent(49.0),
                    );
                    spawn_styled_button(
                        row,
                        &format!("REPLACE P{target_pid} WITH AI"),
                        SidebarAction::CastLagStallVoteAi(target_pid),
                        Color::srgb(0.20, 0.38, 0.52),
                        Val::Percent(49.0),
                    );
                });
            }
        }
    });
}
