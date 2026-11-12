use super::{spawn_body_text, spawn_screen_card};
use crate::input::{ClientInteractionState, MenuScreen};
use crate::ui::studio_layout::{spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_text_input, UiScrollContainer, UiScrollContent, UiTextFieldTarget};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;

pub fn spawn_multiplayer_screen(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    // Column 1: Central Online Lobby Browser & Ranked Automatch
    spawn_screen_card(
        parent,
        35.0,
        "1. ONLINE LOBBY BROWSER & AUTOMATCH",
        Color::srgb(0.62, 0.88, 1.0),
        |col| {
            spawn_text_input(
                col,
                ui_font,
                "Commander Callsign (Cryptographic Profile):",
                UiTextFieldTarget::CommanderName,
                &interaction.player_display_name,
                true,
                false,
            );

            let my_rank = interaction.leaderboard_entries.iter().find(|e| {
                e.player
                    .eq_ignore_ascii_case(interaction.player_display_name.trim())
            });
            let elo_text = if let Some(r) = my_rank {
                format!(
                    "ID: {}  |  Glicko-2 ELO: {:.0} (±{:.0})  |  {}W-{}L (Streak {:+})",
                    interaction.player_identity_short, r.rating, r.rd, r.wins, r.losses, r.streak
                )
            } else {
                format!(
                    "ID: {}  |  Glicko-2 ELO: 1500 (Provisional)  |  Verified Keypair Ready",
                    if interaction.player_identity_short.is_empty() {
                        "auto-provisioned"
                    } else {
                        interaction.player_identity_short.as_str()
                    }
                )
            };
            spawn_body_text(col, elo_text, Color::srgb(0.98, 0.88, 0.50));

            if interaction.automatch_ticket_id.is_some() {
                spawn_body_text(
                    col,
                    format!("AUTOMATCH ACTIVE: {}", interaction.automatch_status_text),
                    Color::srgb(0.45, 0.96, 0.68),
                );
                spawn_styled_button(
                    col,
                    "CANCEL RANKED AUTOMATCH SEARCH",
                    SidebarAction::CancelRankedAutomatch,
                    Color::srgb(0.52, 0.18, 0.18),
                    Val::Percent(100.0),
                );
            } else {
                col.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        "1v1 RANKED AUTOMATCH",
                        SidebarAction::StartRankedAutomatch1v1,
                        Color::srgb(0.20, 0.48, 0.30),
                        Val::Percent(50.0),
                    );
                    spawn_styled_button(
                        row,
                        "2v2 TEAM AUTOMATCH",
                        SidebarAction::StartRankedAutomatch2v2,
                        Color::srgb(0.18, 0.42, 0.52),
                        Val::Percent(50.0),
                    );
                });
            }

            spawn_text_input(
                col,
                ui_font,
                "Central Matchmaking Registry URL:",
                UiTextFieldTarget::LobbyServerUrl,
                &interaction.lobby_server_url,
                true,
                false,
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|row| {
                spawn_styled_button(
                    row,
                    "REFRESH LOBBIES",
                    SidebarAction::RefreshOnlineLobbies,
                    Color::srgb(0.18, 0.38, 0.52),
                    Val::Percent(58.0),
                );
                spawn_styled_button(
                    row,
                    "LOCAL / WAN",
                    SidebarAction::ToggleLobbyServerEndpoint,
                    Color::srgb(0.24, 0.28, 0.42),
                    Val::Percent(42.0),
                );
            });

            let max_scroll = (interaction.online_lobbies.len() as f32 * 34.0 - 140.0).max(0.0);
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
                    if let Some((ref missing_mod_id, ref join_addr)) =
                        interaction.pending_missing_mod_prompt
                    {
                        list.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(6.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.36, 0.18, 0.08, 0.95)),
                            BorderColor(Color::srgb(0.98, 0.76, 0.30)),
                        ))
                        .with_children(|prompt| {
                            spawn_body_text(
                                prompt,
                                format!("Missing Mod '{missing_mod_id}'! Download from Workshop & auto-join?"),
                                Color::srgb(1.0, 0.92, 0.62),
                            );
                            prompt
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|prow| {
                                    spawn_styled_button(
                                        prow,
                                        "DOWNLOAD & JOIN",
                                        SidebarAction::DownloadWorkshopItem {
                                            item_id: missing_mod_id.clone(),
                                            sha256: None,
                                            auto_join_addr: Some(join_addr.clone()),
                                        },
                                        Color::srgb(0.20, 0.48, 0.30),
                                        Val::Percent(68.0),
                                    );
                                    spawn_styled_button(
                                        prow,
                                        "CANCEL",
                                        SidebarAction::DismissMissingModPrompt,
                                        Color::srgb(0.42, 0.18, 0.20),
                                        Val::Percent(32.0),
                                    );
                                });
                        });
                    }
                    if interaction.online_lobbies.is_empty() {
                        spawn_body_text(
                            list,
                            "No public lobbies listed. Click REFRESH LOBBIES, use AUTOMATCH, or host a room.",
                            Color::srgb(0.68, 0.78, 0.88),
                        );
                    }
                    for entry in &interaction.online_lobbies {
                        let pw_tag = if entry.password_protected { "[PW] " } else { "" };
                        let join_target = entry
                            .candidate_connect_addrs()
                            .into_iter()
                            .next()
                            .unwrap_or_else(|| entry.host_addr.clone());
                        let is_live =
                            entry.status == rts_network::lobby_registry::LobbyStatus::InGame;
                        list.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|lrow| {
                            spawn_styled_button(
                                lrow,
                                &format!(
                                    "{}{} ({}/{}) — {} [{}]",
                                    pw_tag,
                                    entry.lobby_name,
                                    entry.current_players,
                                    entry.max_players,
                                    entry.map_name,
                                    if is_live { "LIVE" } else { entry.nat_mode.as_str() }
                                ),
                                SidebarAction::JoinOnlineLobby(join_target),
                                if is_live {
                                    Color::srgb(0.24, 0.30, 0.40)
                                } else {
                                    Color::srgb(0.18, 0.32, 0.46)
                                },
                                Val::Percent(74.0),
                            );
                            spawn_styled_button(
                                lrow,
                                "SPECTATE",
                                SidebarAction::SpectateOnlineLobby(entry.lobby_id.clone()),
                                Color::srgb(0.28, 0.24, 0.48),
                                Val::Percent(26.0),
                            );
                        });
                    }
                });
            });
        },
    );

    // Column 2: Host / Join 8-Slot Staging Room + Password + UDP/UPnP/Relay
    spawn_screen_card(
        parent,
        33.0,
        "2. STAGING ROOM & UDP+TCP LOCKSTEP",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            spawn_body_text(
                col,
                format!(
                    "Mod Hash: {:016x} ({})\nUDP Fast-Path (K=4) + UPnP IGD Lease + Single-Port #mux Relay",
                    loaded.content_hash(),
                    loaded.manifest.id
                ),
                Color::srgb(0.76, 0.90, 0.96),
            );

            spawn_text_input(
                col,
                ui_font,
                "Target Host / Relay Endpoint (IP:Port or host:port#mux:tok):",
                UiTextFieldTarget::ServerAddress,
                &interaction.server_addr_input,
                true,
                interaction.ip_input_editing,
            );

            spawn_text_input(
                col,
                ui_font,
                "Optional Lobby Password (Host sets / Joiner authenticates):",
                UiTextFieldTarget::LobbyPassword,
                &interaction.lobby_password_input,
                true,
                false,
            );

            spawn_styled_button(
                col,
                &format!(
                    "Open Human Slots: {} Players (Click to Cycle 2/4/8)",
                    interaction.host_player_slots
                ),
                SidebarAction::CycleHostPlayerSlots,
                Color::srgb(0.22, 0.30, 0.42),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "HOST 8-SLOT STAGING ROOM (UDP+TCP)",
                SidebarAction::HostLocalMultiplayer,
                Color::srgb(0.18, 0.46, 0.28),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                "JOIN STAGING ROOM / ACTIVE MATCH",
                SidebarAction::JoinLocalMultiplayer,
                Color::srgb(0.18, 0.38, 0.54),
                Val::Percent(100.0),
            );
            if interaction.active_lobby_room.is_some() {
                spawn_styled_button(
                    col,
                    "OPEN ACTIVE STAGING ROOM >>",
                    SidebarAction::NavigateMenu(MenuScreen::MultiplayerStaging),
                    Color::srgb(0.32, 0.52, 0.24),
                    Val::Percent(100.0),
                );
            }
            spawn_body_text(
                col,
                format!("Network Status: {}", interaction.net_status_line),
                Color::srgb(0.65, 0.95, 0.75),
            );
        },
    );

    // Column 3: Glicko-2 Leaderboard, Delayed Spectator & Auto-Reconnect
    spawn_screen_card(
        parent,
        32.0,
        "3. LADDER, SPECTATOR & RECONNECT",
        Color::srgb(0.58, 0.95, 0.74),
        |col| {
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|hdr| {
                spawn_body_text(
                    hdr,
                    "Glicko-2 Ranked Top Commanders:",
                    Color::srgb(0.98, 0.88, 0.52),
                );
                spawn_styled_button(
                    hdr,
                    "REFRESH",
                    SidebarAction::RefreshLeaderboard,
                    Color::srgb(0.18, 0.34, 0.46),
                    Val::Px(84.0),
                );
            });

            if interaction.leaderboard_entries.is_empty() {
                spawn_body_text(
                    col,
                    "Click REFRESH to query live Glicko-2 ladder standings.",
                    Color::srgb(0.70, 0.80, 0.90),
                );
            } else {
                for (idx, rank) in interaction.leaderboard_entries.iter().take(5).enumerate() {
                    spawn_body_text(
                        col,
                        format!(
                            "#{idx_1} {} — {:.0} ELO ({}W-{}L, {:+})",
                            rank.player,
                            rank.rating,
                            rank.wins,
                            rank.losses,
                            rank.streak,
                            idx_1 = idx + 1
                        ),
                        Color::srgb(0.82, 0.94, 0.86),
                    );
                }
            }

            spawn_styled_button(
                col,
                "JOIN AS DELAYED SPECTATOR (30s)",
                SidebarAction::JoinSpectatorMatch,
                Color::srgb(0.26, 0.28, 0.48),
                Val::Percent(100.0),
            );

            let token_status = match interaction.reconnect_token_cached {
                Some(tok) => format!(
                    "Reconnect Token: CACHED ({:08x}) — Auto-Recovery Ready",
                    tok as u32
                ),
                None => "Reconnect Token: Assigned automatically on ServerWelcome".to_string(),
            };
            spawn_body_text(col, token_status, Color::srgb(0.78, 0.88, 0.96));
            spawn_styled_button(
                col,
                "HOT-RECONNECT TO SESSION NOW",
                SidebarAction::TriggerHotReconnect,
                Color::srgb(0.38, 0.28, 0.16),
                Val::Percent(100.0),
            );
            spawn_body_text(
                col,
                &interaction.custom_map_transfer_status,
                Color::srgb(0.64, 0.95, 0.78),
            );
        },
    );
}
