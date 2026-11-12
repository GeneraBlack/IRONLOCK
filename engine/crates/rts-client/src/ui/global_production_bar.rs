use crate::input::{AppPhase, ClientInteractionState};
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::{ButtonVisualStyle, HudLayoutRole, HudPanelBlocker, SidebarAction};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::manifest::{EconomyModel, HudLayoutPreset};
use rts_simulation::world::EntityKind;

#[derive(Component)]
pub struct GlobalProductionStripContent;

#[derive(Default)]
pub struct GlobalProductionSigCache(pub String);

pub fn spawn_global_production_bar(commands: &mut Commands) {
    commands
        .spawn((
            HudLayoutRole::GlobalProductionStripRoot,
            HudPanelBlocker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                right: Val::Px(278.0),
                top: Val::Px(38.0),
                display: Display::None,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.91)),
            BorderColor(Color::srgba(0.30, 0.56, 0.84, 0.82)),
            ZIndex(24),
        ))
        .with_children(|root| {
            root.spawn((
                GlobalProductionStripContent,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ));
        });
}

#[derive(Clone, Debug)]
struct QueueBadgeItem {
    label: String,
    pct: f32,
    bar_color: Color,
    cancel_action: Option<SidebarAction>,
}

/// Persistent Global Production, Era/Age Progression, and Streaming Economy Strip (`Task 240`).
#[allow(clippy::too_many_arguments)]
pub fn update_global_production_bar(
    mut commands: Commands,
    interaction: Res<ClientInteractionState>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    mut sig_cache: Local<GlobalProductionSigCache>,
    mut root_q: Query<(&mut Node, &HudLayoutRole)>,
    content_q: Query<Entity, With<GlobalProductionStripContent>>,
) {
    let show_hud = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let pid = interaction.local_player_id;
    let lang = interaction.language.as_str();
    let econ_opt = world.players.get(&pid);
    let preset = interaction.effective_hud_layout(loaded);

    let is_streaming = loaded.manifest.rules.economy_model == EconomyModel::Streaming;
    let has_eras = !loaded.manifest.rules.eras.is_empty();
    let current_era_id = econ_opt.and_then(|e| e.current_era.clone()).or_else(|| {
        loaded
            .manifest
            .rules
            .eras
            .iter()
            .min_by_key(|er| er.order)
            .map(|er| er.id.clone())
    });

    let current_era_def = current_era_id
        .as_deref()
        .and_then(|id| loaded.manifest.rules.eras.iter().find(|er| er.id == id));
    let next_era_def = if has_eras {
        let cur_idx = current_era_def.map(|e| e.order).unwrap_or(0);
        let mut candidates: Vec<_> = loaded
            .manifest
            .rules
            .eras
            .iter()
            .filter(|er| er.order > cur_idx && Some(er.id.as_str()) != current_era_id.as_deref())
            .collect();
        candidates.sort_by_key(|er| er.order);
        candidates.into_iter().next()
    } else {
        None
    };

    let mut era_advancing_in_flight = false;
    let mut badges: Vec<QueueBadgeItem> = Vec::new();

    if let Some(ec) = econ_opt {
        if let Some(ref ready_id) = ec.ready_building_id {
            let b_name = loaded
                .buildings
                .get(ready_id)
                .map(|b| b.name.as_str())
                .unwrap_or(ready_id);
            badges.push(QueueBadgeItem {
                label: format!("READY: {b_name}"),
                pct: 100.0,
                bar_color: Color::srgb(0.98, 0.86, 0.28),
                cancel_action: Some(SidebarAction::StartPlaceBuilding(ready_id.clone())),
            });
        } else if let Some((ref bid, rem, total)) = ec.building_queue {
            let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(1.0, 99.0);
            let b_name = loaded
                .buildings
                .get(bid)
                .map(|b| b.name.as_str())
                .unwrap_or(bid);
            badges.push(QueueBadgeItem {
                label: format!("BLDG {b_name} {pct:.0}%"),
                pct,
                bar_color: Color::srgb(0.24, 0.90, 0.46),
                cancel_action: None,
            });
        }

        if let Some((ref up_id, rem, total)) = ec.active_upgrade {
            let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(1.0, 99.0);
            let up_name = loaded
                .upgrades
                .get(up_id)
                .map(|u| u.name.as_str())
                .unwrap_or(up_id);
            badges.push(QueueBadgeItem {
                label: format!("TECH {up_name} {pct:.0}%"),
                pct,
                bar_color: Color::srgb(0.36, 0.78, 1.0),
                cancel_action: None,
            });
        }
    }

    // Parallel unit production queues & per-building local research queues
    for ent in world.entities.values() {
        if ent.player_id != pid || ent.health <= 0.0 {
            continue;
        }
        if let Some(head) = ent.production_queue.first() {
            let pct =
                ((1.0 - head.remaining_secs / head.total_secs.max(0.1)) * 100.0).clamp(1.0, 99.0);
            let u_name = loaded
                .units
                .get(&head.unit_id)
                .map(|u| u.name.as_str())
                .unwrap_or(&head.unit_id);
            let cnt = ent.production_queue.len();
            badges.push(QueueBadgeItem {
                label: if cnt > 1 {
                    format!("{u_name} x{cnt} {pct:.0}%")
                } else {
                    format!("{u_name} {pct:.0}%")
                },
                pct,
                bar_color: if head.on_hold {
                    Color::srgb(0.86, 0.64, 0.22)
                } else {
                    Color::srgb(0.28, 0.86, 0.64)
                },
                cancel_action: None,
            });
        }

        if let Some(prod_state) = world.trait_stores.production.get(&ent.id) {
            for job in &prod_state.local_research_queue {
                if job.is_era_advance {
                    era_advancing_in_flight = true;
                }
                let pct =
                    ((1.0 - job.remaining_secs / job.total_secs.max(0.1)) * 100.0).clamp(1.0, 99.0);
                let title = if job.is_era_advance {
                    loaded
                        .manifest
                        .rules
                        .eras
                        .iter()
                        .find(|e| e.id == job.upgrade_id)
                        .map(|e| {
                            if e.display_name_key.is_empty() {
                                e.id.as_str()
                            } else {
                                loaded.t(lang, &e.display_name_key)
                            }
                        })
                        .unwrap_or(&job.upgrade_id)
                } else {
                    loaded
                        .upgrades
                        .get(&job.upgrade_id)
                        .map(|u| u.name.as_str())
                        .unwrap_or(&job.upgrade_id)
                };
                badges.push(QueueBadgeItem {
                    label: format!(
                        "{}: {} {:.0}% [X]",
                        if job.is_era_advance { "ERA" } else { "LAB" },
                        title,
                        pct
                    ),
                    pct,
                    bar_color: Color::srgb(0.84, 0.56, 1.0),
                    cancel_action: Some(SidebarAction::CancelLocalResearch {
                        building_id: ent.id,
                        upgrade_id: job.upgrade_id.clone(),
                    }),
                });
            }
        }

        if let Some(rev_q) = world.trait_stores.revival_queues.get(&ent.id) {
            if let Some(rev) = rev_q.front() {
                let pct =
                    ((1.0 - rev.remaining_secs / rev.total_secs.max(0.1)) * 100.0).clamp(1.0, 99.0);
                let h_name = loaded
                    .units
                    .get(&rev.record.unit_id)
                    .map(|u| u.name.as_str())
                    .unwrap_or(&rev.record.unit_id);
                badges.push(QueueBadgeItem {
                    label: format!("REVIVE {h_name} {pct:.0}%"),
                    pct,
                    bar_color: Color::srgb(1.0, 0.78, 0.28),
                    cancel_action: None,
                });
            }
        }

        if matches!(ent.kind, EntityKind::Building { .. }) {
            if let Some((ref target_b, rem, total)) = ent.upgrading_to {
                let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(1.0, 99.0);
                let b_name = loaded
                    .buildings
                    .get(target_b)
                    .map(|b| b.name.as_str())
                    .unwrap_or(target_b);
                badges.push(QueueBadgeItem {
                    label: format!("UPG->{b_name} {pct:.0}%"),
                    pct,
                    bar_color: Color::srgb(0.36, 0.88, 0.92),
                    cancel_action: None,
                });
            }
        }
    }

    let should_display = show_hud
        && (is_streaming
            || has_eras
            || !badges.is_empty()
            || preset != HudLayoutPreset::ClassicRightSidebar);

    for (mut node, role) in &mut root_q {
        if *role == HudLayoutRole::GlobalProductionStripRoot {
            node.display = if should_display {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    let stream_summary = if is_streaming {
        let inc = econ_opt
            .and_then(|e| e.income_per_sec.get("credits").copied())
            .unwrap_or(0.0);
        let exp = econ_opt
            .and_then(|e| e.expense_per_sec.get("credits").copied())
            .unwrap_or(0.0);
        let eff = (econ_opt.map(|e| e.stall_efficiency).unwrap_or(1.0) * 100.0).round() as i32;
        format!("+{inc:.1}/s  -{exp:.1}/s  ({eff}% {})", tr(lang, "hud_eff"))
    } else {
        String::new()
    };

    let sig = format!(
        "show={should_display}|preset={preset:?}|lang={lang}|era={current_era_id:?}|next={:?}|adv={era_advancing_in_flight}|str={stream_summary}|b={}",
        next_era_def.map(|e| &e.id),
        badges
            .iter()
            .map(|b| format!("{}:{}", b.label, (b.pct as u32) / 2))
            .collect::<Vec<_>>()
            .join("|")
    );

    if sig == sig_cache.0 {
        return;
    }
    sig_cache.0 = sig;

    let Ok(content_ent) = content_q.get_single() else {
        return;
    };
    commands.entity(content_ent).despawn_descendants();
    if !should_display {
        return;
    }

    commands.entity(content_ent).with_children(|row| {
        // Quick HUD Layout Preset Pill
        let layout_short = match preset {
            HudLayoutPreset::ClassicRightSidebar => "HUD: CLASSIC",
            HudLayoutPreset::ModernBottomCommandCard => "HUD: BOTTOM",
            HudLayoutPreset::CompactBarOverlay => "HUD: COMPACT",
        };
        spawn_styled_button(
            row,
            layout_short,
            SidebarAction::CycleHudLayoutPreset,
            Color::srgb(0.16, 0.24, 0.36),
            Val::Auto,
        );

        // Era / Age Badge + `[Advance Era]`
        if has_eras {
            let era_title = current_era_def
                .map(|e| {
                    if e.display_name_key.is_empty() {
                        e.id.as_str()
                    } else {
                        loaded.t(lang, &e.display_name_key)
                    }
                })
                .or(current_era_id.as_deref())
                .unwrap_or("Age I");
            row.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.22, 0.16, 0.06, 0.95)),
                BorderColor(Color::srgb(0.94, 0.76, 0.28)),
            ))
            .with_children(|pill| {
                pill.spawn((
                    Text::new(sanitize_ui_text(&format!(
                        "{}: {}",
                        tr(lang, "hud_era"),
                        era_title
                    ))),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.90, 0.48)),
                ));
            });

            if let Some(next_era) = next_era_def {
                if !era_advancing_in_flight {
                    let next_lbl = if next_era.display_name_key.is_empty() {
                        next_era.id.as_str()
                    } else {
                        loaded.t(lang, &next_era.display_name_key)
                    };
                    spawn_styled_button(
                        row,
                        &format!(
                            "{} -> {} (${})",
                            tr(lang, "hud_advance_era"),
                            next_lbl,
                            next_era.cost
                        ),
                        SidebarAction::AdvancePlayerEra(next_era.id.clone()),
                        Color::srgb(0.46, 0.34, 0.12),
                        Val::Auto,
                    );
                }
            }
        }

        // Streaming Economy Meter
        if is_streaming {
            let eff = econ_opt.map(|e| e.stall_efficiency).unwrap_or(1.0);
            let meter_col = if eff >= 0.95 {
                Color::srgb(0.42, 0.96, 0.62)
            } else if eff >= 0.60 {
                Color::srgb(0.98, 0.84, 0.30)
            } else {
                Color::srgb(0.98, 0.36, 0.34)
            };
            row.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.12, 0.16, 0.94)),
                BorderColor(meter_col),
            ))
            .with_children(|m| {
                m.spawn((
                    Text::new(sanitize_ui_text(&format!("FLOW {stream_summary}"))),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(meter_col),
                ));
            });
        }

        // Parallel Active Queue Badges with Progress Bars
        for badge in badges.into_iter().take(6) {
            let mut card = row.spawn((
                Node {
                    min_width: Val::Px(112.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.5)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.11, 0.18, 0.95)),
                BorderColor(badge.bar_color),
            ));
            if let Some(act) = badge.cancel_action {
                card.insert((
                    Button,
                    act,
                    ButtonVisualStyle {
                        normal: Color::srgba(0.07, 0.11, 0.18, 0.95),
                        hovered: Color::srgba(0.24, 0.14, 0.22, 0.98),
                        pressed: Color::srgba(0.36, 0.16, 0.24, 1.0),
                        default_border: badge.bar_color,
                    },
                ));
            }
            card.with_children(|c| {
                c.spawn((
                    Text::new(sanitize_ui_text(&badge.label)),
                    TextFont {
                        font_size: 9.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.96, 1.0)),
                ));
                c.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(3.5),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.14, 0.22)),
                ))
                .with_children(|track| {
                    track.spawn((
                        Node {
                            width: Val::Percent(badge.pct.clamp(2.0, 100.0)),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(badge.bar_color),
                    ));
                });
            });
        }
    });
}
