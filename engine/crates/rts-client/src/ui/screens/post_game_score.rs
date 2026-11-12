use super::{spawn_body_text, spawn_screen_card};
use crate::input::ClientInteractionState;
use crate::rendering::VideoPlaybackResource;
use crate::ui::locale::ui_t;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

pub fn spawn_post_game_score_screen(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &ClientInteractionState,
    video_res: Option<&VideoPlaybackResource>,
    graph_img: Option<Handle<Image>>,
) {
    let local_pid = interaction.local_player_id;
    let winner_opt = world.winner_player_id;
    let is_victory = winner_opt.is_none_or(|w| w == local_pid || !world.are_hostile(local_pid, w));
    let banner_key = if is_victory {
        "ui.postgame.victory"
    } else {
        "ui.postgame.defeat"
    };
    let banner_title = ui_t(loaded, interaction, banner_key);
    let banner_color = if is_victory {
        Color::srgb(0.36, 0.96, 0.56)
    } else {
        Color::srgb(1.0, 0.36, 0.34)
    };

    let has_next_campaign = interaction
        .active_campaign_id
        .as_ref()
        .and_then(|cid| loaded.campaigns.get(cid))
        .and_then(|c| {
            interaction
                .active_mission_id
                .as_ref()
                .and_then(|mid| c.missions.iter().find(|m| &m.id == mid))
        })
        .and_then(|m| m.next_mission_id.as_ref())
        .is_some();

    spawn_screen_card(parent, 100.0, &banner_title, banner_color, |col| {
        let mins = (world.tick / 20) / 60;
        let secs = (world.tick / 20) % 60;
        col.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|hdr| {
            spawn_body_text(
                hdr,
                format!(
                    "Sector: {}  |  Duration: {:02}:{:02} ({} ticks)  |  Winner: Player {}",
                    loaded.entry_map.name,
                    mins,
                    secs,
                    world.tick,
                    winner_opt.unwrap_or(local_pid)
                ),
                Color::srgb(0.84, 0.92, 0.98),
            );
            if let Some(vid) = video_res.filter(|v| v.is_playing) {
                hdr.spawn((
                    ImageNode::new(vid.frame_image.clone()),
                    Node {
                        width: Val::Px(136.0),
                        height: Val::Px(74.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BorderColor(banner_color),
                ));
            }
        });

        // 4-Tab Selector
        col.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|tabs| {
            let tab_defs = [
                (0u8, ui_t(loaded, interaction, "ui.postgame.tab_summary")),
                (1u8, ui_t(loaded, interaction, "ui.postgame.tab_graphs")),
                (2u8, ui_t(loaded, interaction, "ui.postgame.tab_ledger")),
                (3u8, ui_t(loaded, interaction, "ui.postgame.tab_timeline")),
            ];
            for (idx, lbl) in tab_defs {
                let active = interaction.post_game_tab == idx;
                spawn_styled_button(
                    tabs,
                    &lbl,
                    SidebarAction::SetPostGameTab(idx),
                    if active {
                        Color::srgb(0.22, 0.46, 0.64)
                    } else {
                        Color::srgb(0.14, 0.20, 0.30)
                    },
                    Val::Percent(25.0),
                );
            }
        });

        // Tab Content Body
        col.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.92)),
            BorderColor(Color::srgba(0.36, 0.54, 0.78, 0.75)),
        ))
        .with_children(|body| match interaction.post_game_tab {
            0 => spawn_summary_and_mvp_tab(body, loaded, world),
            1 => spawn_graphs_tab(body, world, graph_img),
            2 => spawn_combat_ledger_tab(body, loaded, world),
            _ => spawn_build_timeline_tab(body, loaded, world),
        });

        // Action Footer
        col.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|footer| {
            spawn_styled_button(
                footer,
                "CONTINUE VIEWING BATTLEFIELD",
                SidebarAction::ContinueViewingBattlefield,
                Color::srgb(0.18, 0.40, 0.52),
                Val::Percent(27.0),
            );
            spawn_styled_button(
                footer,
                "SAVE REPLAY (.RTSREP)",
                SidebarAction::SaveMatchReplayNow,
                Color::srgb(0.24, 0.32, 0.48),
                Val::Percent(24.0),
            );
            if is_victory && has_next_campaign {
                spawn_styled_button(
                    footer,
                    "NEXT CAMPAIGN MISSION >>",
                    SidebarAction::AdvanceNextCampaignMission,
                    Color::srgb(0.18, 0.52, 0.28),
                    Val::Percent(25.0),
                );
            } else if !interaction.is_multiplayer {
                spawn_styled_button(
                    footer,
                    "REMATCH SECTOR",
                    SidebarAction::RestartMatch,
                    Color::srgb(0.20, 0.46, 0.28),
                    Val::Percent(25.0),
                );
            }
            spawn_styled_button(
                footer,
                &ui_t(loaded, interaction, "ui.menu.return_main"),
                SidebarAction::ReturnToMainMenu,
                Color::srgb(0.46, 0.18, 0.20),
                Val::Percent(24.0),
            );
        });
    });
}

fn spawn_summary_and_mvp_tab(body: &mut ChildBuilder, loaded: &LoadedMod, world: &SimWorld) {
    // MVP Unit Spotlight (highest total damage_dealt across all players)
    let mvp = world
        .telemetry
        .per_unit_ledger
        .iter()
        .flat_map(|(&pid, map)| map.iter().map(move |(uid, led)| (pid, uid, led)))
        .max_by(|a, b| a.2.damage_dealt.total_cmp(&b.2.damage_dealt));

    if let Some((mvp_pid, mvp_id, led)) = mvp {
        let name = loaded
            .units
            .get(mvp_id)
            .map(|u| u.name.as_str())
            .or_else(|| loaded.buildings.get(mvp_id).map(|b| b.name.as_str()))
            .unwrap_or(mvp_id.as_str());
        spawn_body_text(
            body,
            format!(
                "BATTLEFIELD MVP SPOTLIGHT: Player {mvp_pid}'s {name} — {:.0} Damage Dealt  |  {} Kills  |  {} Deployed",
                led.damage_dealt, led.killed, led.built
            ),
            Color::srgb(0.98, 0.88, 0.44),
        );
    }

    for (&pid, econ) in &world.players {
        if pid == 0 {
            continue;
        }
        let ledger_map = world.telemetry.per_unit_ledger.get(&pid);
        let (built, kills, lost, dmg) = ledger_map
            .map(|m| {
                m.values().fold((0u32, 0u32, 0u32, 0.0f32), |acc, l| {
                    (
                        acc.0 + l.built,
                        acc.1 + l.killed,
                        acc.2 + l.lost,
                        acc.3 + l.damage_dealt,
                    )
                })
            })
            .unwrap_or((0, 0, 0, 0.0));
        let kd = if lost == 0 {
            kills as f32
        } else {
            kills as f32 / lost as f32
        };
        let peak_apm = world
            .telemetry
            .player_samples
            .get(&pid)
            .and_then(|s| s.iter().map(|x| x.apm).max())
            .unwrap_or(0);
        spawn_body_text(
            body,
            format!(
                "Player {pid} [{} | Team {}] — Credits: ${}  |  Built: {built}  |  Kills: {kills}  |  Lost: {lost} (K/D {kd:.2})  |  Dmg: {dmg:.0}  |  Peak APM: {peak_apm}",
                econ.faction, econ.team_id, econ.credits
            ),
            Color::srgb(0.84, 0.94, 0.98),
        );
    }
}

fn spawn_graphs_tab(body: &mut ChildBuilder, world: &SimWorld, graph_img: Option<Handle<Image>>) {
    spawn_body_text(
        body,
        "Top-Left: Economy Credits & Income/min  |  Top-Right: Standing Army Value  |  Bottom-Left: Net Power  |  Bottom-Right: Commander APM",
        Color::srgb(0.78, 0.90, 0.98),
    );
    if let Some(handle) = graph_img {
        body.spawn((
            ImageNode::new(handle),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(250.0),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BorderColor(Color::srgb(0.36, 0.58, 0.84)),
        ));
    }
    let legend: Vec<String> = world
        .players
        .iter()
        .filter(|(&pid, _)| pid > 0)
        .map(|(&pid, ec)| {
            let n = world
                .telemetry
                .player_samples
                .get(&pid)
                .map(|s| s.len())
                .unwrap_or(0);
            format!("P{pid} ({}) [{} samples]", ec.faction, n)
        })
        .collect();
    spawn_body_text(body, legend.join("   |   "), Color::srgb(0.96, 0.86, 0.48));
}

fn spawn_combat_ledger_tab(body: &mut ChildBuilder, loaded: &LoadedMod, world: &SimWorld) {
    let mut rows: Vec<String> = Vec::new();
    for (&pid, umap) in &world.telemetry.per_unit_ledger {
        for (uid, l) in umap {
            let name = loaded
                .units
                .get(uid)
                .map(|u| u.name.as_str())
                .or_else(|| loaded.buildings.get(uid).map(|b| b.name.as_str()))
                .unwrap_or(uid.as_str());
            rows.push(format!(
                "P{pid} — {name:<24} | Built: {:>3} | Killed: {:>3} | Lost: {:>3} | Damage Dealt: {:>7.0}",
                l.built, l.killed, l.lost, l.damage_dealt
            ));
        }
    }
    spawn_scrollable_lines(
        body,
        &rows,
        "No unit combat engagements recorded in this sector yet.",
    );
}

fn spawn_build_timeline_tab(body: &mut ChildBuilder, loaded: &LoadedMod, world: &SimWorld) {
    let rows: Vec<String> = world
        .telemetry
        .build_order_events
        .iter()
        .map(|ev| {
            let secs = ev.tick / 20;
            let name = loaded
                .buildings
                .get(&ev.item_id)
                .map(|b| b.name.as_str())
                .or_else(|| loaded.units.get(&ev.item_id).map(|u| u.name.as_str()))
                .unwrap_or(ev.item_id.as_str());
            let kind_lbl = if ev.is_building {
                "constructed"
            } else {
                "deployed"
            };
            format!(
                "[{:02}:{:02}] Player {} {} {}",
                secs / 60,
                secs % 60,
                ev.player_id,
                kind_lbl,
                name
            )
        })
        .collect();
    spawn_scrollable_lines(body, &rows, "No build-order milestones recorded yet.");
}

fn spawn_scrollable_lines(body: &mut ChildBuilder, rows: &[String], empty_msg: &str) {
    let max_scroll = (rows.len() as f32 * 20.0 - 220.0).max(0.0);
    body.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(260.0),
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
                row_gap: Val::Px(3.0),
                ..default()
            },
        ))
        .with_children(|list| {
            if rows.is_empty() {
                list.spawn((
                    Text::new(empty_msg),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.70, 0.80, 0.90)),
                ));
            }
            for r in rows {
                list.spawn((
                    Text::new(sanitize_ui_text(r)),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.93, 0.98)),
                ));
            }
        });
    });
}
