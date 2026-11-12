use super::{spawn_artwork_banner, spawn_body_text, spawn_screen_card};
use crate::input::{resolve_mod_skirmish_descriptor, ClientInteractionState, UiAssetCache};
use crate::ui::locale::ui_t;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_text_input, UiScrollContainer, UiScrollContent, UiTextFieldTarget};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_network::lobby_room::{LobbyMatchRules, LobbyRoomState, LobbySlotOccupant};
use rts_sdk::loader::LoadedMod;

pub const STAGING_SLOT_COLORS: [(f32, f32, f32, &str); 8] = [
    (0.22, 0.56, 0.96, "Azure"),
    (0.92, 0.24, 0.24, "Crimson"),
    (0.22, 0.82, 0.38, "Emerald"),
    (0.96, 0.80, 0.18, "Gold"),
    (0.72, 0.34, 0.94, "Violet"),
    (0.18, 0.86, 0.88, "Cyan"),
    (0.96, 0.52, 0.16, "Orange"),
    (0.92, 0.44, 0.76, "Magenta"),
];

fn team_label(team_id: u8) -> &'static str {
    match team_id {
        0 => "FFA",
        1 => "Team A",
        2 => "Team B",
        3 => "Team C",
        _ => "Team D",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_mp_staging_room_screen(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    save_files: &[std::path::PathBuf],
) {
    let fallback_room = {
        let mut r = LobbyRoomState::new(
            interaction.host_player_slots.clamp(2, 8),
            LobbyMatchRules {
                map_id: loaded.entry_map.id.clone(),
                map_name: loaded.entry_map.name.clone(),
                ..Default::default()
            },
        );
        let _ = r.assign_human(1, interaction.player_display_name.clone());
        r
    };
    let room = interaction
        .active_lobby_room
        .as_ref()
        .unwrap_or(&fallback_room);
    let local_pid = interaction.local_player_id;
    let is_host = local_pid == room.host_player_id;

    // Column 1 (48%): 8-Slot Synchronized Player/AI Grid
    let title = ui_t(loaded, interaction, "ui.staging.title");
    spawn_screen_card(parent, 48.0, &title, Color::srgb(0.98, 0.86, 0.40), |col| {
        spawn_body_text(
            col,
            format!(
                "Role: {} (P{local_pid})  |  Room Rev #{}  |  Status: {}",
                if is_host {
                    "HOST COMMANDER"
                } else {
                    "PEER COMMANDER"
                },
                room.revision,
                interaction.net_status_line
            ),
            Color::srgb(0.76, 0.92, 0.98),
        );

        for slot in &room.slots {
            let s_idx = slot.slot_index;
            let occ_label = match &slot.occupant {
                LobbySlotOccupant::Human { player_id, name } => {
                    format!("S{}: {} (P{})", s_idx + 1, name, player_id)
                }
                LobbySlotOccupant::AiBot {
                    difficulty,
                    personality,
                } => format!("S{}: AI [{}/{}]", s_idx + 1, difficulty, personality),
                LobbySlotOccupant::Open => format!("S{}: [OPEN SLOT]", s_idx + 1),
                LobbySlotOccupant::Closed => format!("S{}: [CLOSED]", s_idx + 1),
            };

            let is_active_slot = matches!(
                slot.occupant,
                LobbySlotOccupant::Human { .. } | LobbySlotOccupant::AiBot { .. }
            );
            let (cr, cg, cb, cname) =
                STAGING_SLOT_COLORS[(slot.color_index as usize) % STAGING_SLOT_COLORS.len()];
            let fac_label = if slot.faction_id.eq_ignore_ascii_case("random") {
                "Random".to_string()
            } else {
                loaded
                    .factions
                    .get(&slot.faction_id)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| slot.faction_id.clone())
            };
            let sub_label = slot.sub_faction_id.as_deref().unwrap_or("Std");
            let sp_label = slot
                .spawn_index
                .map(|sp| format!("Sp{}", sp + 1))
                .unwrap_or_else(|| "Auto".to_string());
            let ready_badge = match &slot.occupant {
                LobbySlotOccupant::Human { player_id, .. } => {
                    if !slot.map_ready {
                        "DL MAP"
                    } else if room.rules.resume_save_hash.is_some() && !slot.save_ready {
                        "SYNC SAVE"
                    } else if *player_id == room.host_player_id {
                        "HOST"
                    } else if slot.ready {
                        "READY"
                    } else {
                        "WAIT"
                    }
                }
                LobbySlotOccupant::AiBot { .. } => "AI OK",
                LobbySlotOccupant::Open => "OPEN",
                LobbySlotOccupant::Closed => "OFF",
            };

            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.09, 0.15, 0.92)),
                BorderColor(if is_active_slot {
                    Color::srgb(cr, cg, cb)
                } else {
                    Color::srgba(0.30, 0.38, 0.50, 0.55)
                }),
            ))
            .with_children(|row| {
                spawn_styled_button(
                    row,
                    &occ_label,
                    SidebarAction::StagingCycleSlotOccupant(s_idx),
                    if matches!(slot.occupant, LobbySlotOccupant::Closed) {
                        Color::srgb(0.16, 0.16, 0.20)
                    } else {
                        Color::srgb(0.18, 0.30, 0.44)
                    },
                    Val::Percent(29.0),
                );
                spawn_styled_button(
                    row,
                    &format!("{fac_label}/{sub_label}"),
                    SidebarAction::StagingCycleSlotFaction(s_idx),
                    Color::srgb(0.20, 0.34, 0.32),
                    Val::Percent(23.0),
                );
                spawn_styled_button(
                    row,
                    cname,
                    SidebarAction::StagingCycleSlotColor(s_idx),
                    Color::srgb(cr * 0.65, cg * 0.65, cb * 0.65),
                    Val::Percent(14.0),
                );
                spawn_styled_button(
                    row,
                    team_label(slot.team_id),
                    SidebarAction::StagingCycleSlotTeam(s_idx),
                    Color::srgb(0.24, 0.26, 0.40),
                    Val::Percent(13.0),
                );
                spawn_styled_button(
                    row,
                    &format!("{}%/{sp_label}", slot.handicap_pct),
                    SidebarAction::StagingCycleSlotHandicap(s_idx),
                    Color::srgb(0.26, 0.24, 0.32),
                    Val::Percent(13.0),
                );
                row.spawn((
                    Text::new(sanitize_ui_text(ready_badge)),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(
                        if ready_badge == "READY" || ready_badge == "HOST" || ready_badge == "AI OK"
                        {
                            Color::srgb(0.42, 0.96, 0.58)
                        } else {
                            Color::srgb(0.95, 0.76, 0.34)
                        },
                    ),
                ));
            });
        }
    });

    // Column 2 (26%): Minimap Spawn Picker [1..8], Match Rules & 32 KB Map Transfer
    spawn_screen_card(
        parent,
        26.0,
        "MAP SPAWNS & MATCH RULES",
        Color::srgb(0.58, 0.90, 1.0),
        |col| {
            let (_, _, _, art) =
                resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
            spawn_artwork_banner(
                col,
                ui_cache,
                asset_server,
                asset_root,
                art.as_deref(),
                82.0,
            );

            spawn_styled_button(
                col,
                &format!("Map: {} ({})", room.rules.map_name, room.rules.map_id),
                SidebarAction::StagingCycleRuleMap,
                Color::srgb(0.18, 0.36, 0.48),
                Val::Percent(100.0),
            );

            spawn_body_text(
                col,
                "Claim Starting Spawn Point [1..8] (or Auto):",
                Color::srgb(0.86, 0.92, 0.98),
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(3.0),
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|sp_grid| {
                let my_sp = room.find_human_slot(local_pid).and_then(|s| s.spawn_index);
                spawn_styled_button(
                    sp_grid,
                    "AUTO",
                    SidebarAction::StagingSelectSpawn(255),
                    if my_sp.is_none() {
                        Color::srgb(0.24, 0.50, 0.32)
                    } else {
                        Color::srgb(0.16, 0.24, 0.34)
                    },
                    Val::Percent(22.0),
                );
                let max_spawns = loaded.entry_map.spawns.len().clamp(2, 8) as u8;
                for sp in 0..max_spawns {
                    let claimed_by = room
                        .slots
                        .iter()
                        .find(|s| s.spawn_index == Some(sp))
                        .map(|s| s.slot_index + 1);
                    let lbl = match claimed_by {
                        Some(s_num) => format!("#{}(S{})", sp + 1, s_num),
                        None => format!("Sp #{}", sp + 1),
                    };
                    spawn_styled_button(
                        sp_grid,
                        &lbl,
                        SidebarAction::StagingSelectSpawn(sp),
                        if my_sp == Some(sp) {
                            Color::srgb(0.24, 0.52, 0.34)
                        } else if claimed_by.is_some() {
                            Color::srgb(0.38, 0.28, 0.16)
                        } else {
                            Color::srgb(0.16, 0.24, 0.36)
                        },
                        Val::Percent(23.5),
                    );
                }
            });

            spawn_styled_button(
                col,
                &format!("Starting Credits: ${}", room.rules.starting_credits),
                SidebarAction::StagingCycleRuleCredits,
                Color::srgb(0.20, 0.32, 0.42),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!(
                    "Mode: {}  |  Sim Speed: {} Hz",
                    room.rules.game_mode, room.rules.game_speed_hz
                ),
                SidebarAction::StagingCycleGameMode,
                Color::srgb(0.20, 0.30, 0.44),
                Val::Percent(100.0),
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|rrow| {
                spawn_styled_button(
                    rrow,
                    if room.rules.superweapons_enabled {
                        "SW: ON"
                    } else {
                        "SW: OFF"
                    },
                    SidebarAction::StagingToggleSuperweapons,
                    Color::srgb(0.24, 0.26, 0.38),
                    Val::Percent(33.0),
                );
                spawn_styled_button(
                    rrow,
                    if room.rules.crates_enabled {
                        "CRATES: ON"
                    } else {
                        "CRATES: OFF"
                    },
                    SidebarAction::StagingToggleCrates,
                    Color::srgb(0.24, 0.26, 0.38),
                    Val::Percent(34.0),
                );
                spawn_styled_button(
                    rrow,
                    if room.rules.fog_of_war {
                        "FOW: ON"
                    } else {
                        "FOW: OFF"
                    },
                    SidebarAction::StagingToggleFogOfWar,
                    Color::srgb(0.24, 0.26, 0.38),
                    Val::Percent(33.0),
                );
            });

            super::mp_staging_rules::spawn_staging_mutators_and_rollback(
                col, ui_cache, loaded, room, is_host,
            );

            super::workshop_screen::spawn_staging_spectator_and_resume_controls(
                col,
                room,
                is_host,
                save_files,
                interaction,
            );

            // Custom Map 32 KB Chunk Progress Bar
            let pct = interaction.custom_map_transfer_pct.clamp(0.0, 100.0);
            spawn_body_text(
                col,
                format!(
                    "Map Sync ({pct:.0}%): {}",
                    interaction.custom_map_transfer_status
                ),
                Color::srgb(0.68, 0.94, 0.78),
            );
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(8.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.10, 0.16)),
                BorderColor(Color::srgb(0.36, 0.58, 0.82)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent(pct),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.24, 0.88, 0.52)),
                ));
            });
            if is_host {
                spawn_styled_button(
                    col,
                    "STREAM CUSTOM MAP TO PEERS",
                    SidebarAction::StagingPublishCustomMap,
                    Color::srgb(0.20, 0.38, 0.38),
                    Val::Percent(100.0),
                );
            }
        },
    );

    // Column 3 (26%): Staging Chat, Ready & Host Launch
    spawn_screen_card(
        parent,
        26.0,
        "LOBBY COMMS & LAUNCH",
        Color::srgb(0.56, 0.96, 0.72),
        |col| {
            let max_scroll =
                (interaction.staging_chat_history.len() as f32 * 20.0 - 180.0).max(0.0);
            col.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    overflow: Overflow::clip_y(),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.90)),
                BorderColor(Color::srgba(0.34, 0.52, 0.74, 0.70)),
                UiScrollContainer::new(max_scroll),
            ))
            .with_children(|vp| {
                vp.spawn((
                    UiScrollContent,
                    Node {
                        width: Val::Percent(100.0),
                        position_type: PositionType::Relative,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ))
                .with_children(|feed| {
                    if interaction.staging_chat_history.is_empty() {
                        spawn_body_text(
                            feed,
                            "Staging Room Radio Ready. Coordinate teams, factions & spawns below.",
                            Color::srgb(0.68, 0.78, 0.88),
                        );
                    }
                    for line in &interaction.staging_chat_history {
                        spawn_body_text(feed, line, Color::srgb(0.86, 0.94, 0.98));
                    }
                });
            });

            spawn_text_input(
                col,
                ui_font,
                "Lobby Chat Message:",
                UiTextFieldTarget::StagingChat,
                &interaction.staging_chat_input,
                true,
                false,
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|crow| {
                spawn_styled_button(
                    crow,
                    "SEND ALL",
                    SidebarAction::StagingSendChat(false),
                    Color::srgb(0.18, 0.36, 0.50),
                    Val::Percent(50.0),
                );
                spawn_styled_button(
                    crow,
                    "SEND TEAM",
                    SidebarAction::StagingSendChat(true),
                    Color::srgb(0.18, 0.42, 0.36),
                    Val::Percent(50.0),
                );
            });

            if !is_host {
                let am_ready = room
                    .find_human_slot(local_pid)
                    .map(|s| s.ready)
                    .unwrap_or(false);
                let ready_txt = if am_ready {
                    "READY CONFIRMED (CLICK TO UNREADY)".to_string()
                } else {
                    ui_t(loaded, interaction, "ui.staging.ready")
                };
                spawn_styled_button(
                    col,
                    &ready_txt,
                    SidebarAction::StagingToggleReady,
                    if am_ready {
                        Color::srgb(0.20, 0.54, 0.32)
                    } else {
                        Color::srgb(0.46, 0.34, 0.14)
                    },
                    Val::Percent(100.0),
                );
            } else {
                let launchable = room.can_launch();
                let launch_txt = if launchable {
                    ui_t(loaded, interaction, "ui.staging.launch")
                } else {
                    "WAITING FOR PEERS READY & MAP SYNC...".to_string()
                };
                spawn_styled_button(
                    col,
                    &launch_txt,
                    SidebarAction::StagingLaunchMatch,
                    if launchable {
                        Color::srgb(0.18, 0.56, 0.28)
                    } else {
                        Color::srgb(0.30, 0.30, 0.34)
                    },
                    Val::Percent(100.0),
                );
            }

            spawn_styled_button(
                col,
                "LEAVE STAGING ROOM",
                SidebarAction::LeaveStagingRoom,
                Color::srgb(0.46, 0.16, 0.18),
                Val::Percent(100.0),
            );
        },
    );
}
