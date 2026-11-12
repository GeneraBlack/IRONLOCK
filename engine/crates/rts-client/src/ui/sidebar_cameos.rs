use super::*;
use crate::input::keymap::GRID_CAMEO_KEY_LABELS;
use crate::input::{ClientInteractionState, ProductionSidebarTab, VehicleDomainFilter};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::manifest::HudLayoutPreset;
use rts_simulation::world::SimWorld;
use std::collections::{BTreeMap, HashMap};

const CAMEOS_PER_PAGE_CLASSIC: usize = 8;
const CAMEOS_PER_PAGE_BOTTOM_4X3: usize = 12;

pub fn cameos_per_page_for_preset(preset: HudLayoutPreset) -> usize {
    match preset {
        HudLayoutPreset::ModernBottomCommandCard => CAMEOS_PER_PAGE_BOTTOM_4X3,
        _ => CAMEOS_PER_PAGE_CLASSIC,
    }
}

fn truncate_cameo_title(name: &str, max_chars: usize) -> String {
    let clean = studio_layout::sanitize_ui_text(name);
    if clean.chars().count() <= max_chars {
        clean
    } else {
        let mut out: String = clean.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('.');
        out
    }
}

fn format_prereqs(loaded: &LoadedMod, prereqs: &[String], empty_fallback: &str) -> String {
    if prereqs.is_empty() {
        return empty_fallback.to_string();
    }
    prereqs
        .iter()
        .map(|id| {
            loaded
                .buildings
                .get(id)
                .map(|b| b.name.as_str())
                .or_else(|| loaded.units.get(id).map(|u| u.name.as_str()))
                .unwrap_or(id.as_str())
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Renders a clean `104x68` sidebar cameo button (`Task 208`):
/// - Top-left compact hotkey/queue badge (`[Q]`, `x3 64%`, `READY`, `ON HOLD`)
/// - Top-right compact cost pill (`$800`)
/// - Slim `3.5px` bottom progress strip leaving baked RA2 PNG bottom labels 100% unobstructed
/// - Text title strip ONLY when `icon_handle.is_none()` (without duplicating `[Q]`).
#[allow(clippy::too_many_arguments)]
pub fn spawn_cameo_grid_button(
    parent: &mut ChildBuilder,
    short_title: &str,
    cost_label: &str,
    status_badge: &str,
    icon_handle: Option<Handle<Image>>,
    action: SidebarAction,
    base_color: Color,
    is_highlighted: bool,
    progress_pct: Option<f32>,
    tooltip: Option<String>,
) {
    spawn_cameo_grid_button_sized(
        parent,
        (104.0, 68.0),
        short_title,
        cost_label,
        status_badge,
        icon_handle,
        action,
        base_color,
        is_highlighted,
        progress_pct,
        tooltip,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_cameo_grid_button_sized(
    parent: &mut ChildBuilder,
    size_px: (f32, f32),
    short_title: &str,
    cost_label: &str,
    status_badge: &str,
    icon_handle: Option<Handle<Image>>,
    action: SidebarAction,
    base_color: Color,
    is_highlighted: bool,
    progress_pct: Option<f32>,
    tooltip: Option<String>,
) {
    let border_col = if is_highlighted {
        Color::srgb(1.0, 0.90, 0.30)
    } else {
        Color::srgba(0.65, 0.82, 1.0, 0.42)
    };

    let mut entity_cmds = parent.spawn((
        Button,
        action,
        ButtonVisualStyle {
            normal: if icon_handle.is_some() {
                Color::WHITE
            } else {
                base_color
            },
            hovered: Color::srgb(0.88, 0.96, 1.0),
            pressed: Color::srgb(0.68, 0.78, 0.92),
            default_border: border_col,
        },
        Node {
            width: Val::Px(size_px.0),
            height: Val::Px(size_px.1),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Stretch,
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(if is_highlighted { 2.0 } else { 1.0 })),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(if icon_handle.is_some() {
            Color::WHITE
        } else {
            base_color
        }),
        BorderColor(border_col),
    ));
    if let Some(tip) = tooltip {
        entity_cmds.insert(CameoTooltip(tip));
    }

    entity_cmds.with_children(|btn| {
        if let Some(img) = icon_handle.clone() {
            btn.spawn((
                ImageNode::new(img),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
        }

        // Top row: top-left hotkey/queue badge + top-right cost pill
        btn.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::FlexStart,
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        })
        .with_children(|top_row| {
            if !status_badge.is_empty() {
                let badge_bg = if status_badge.starts_with("ON HOLD") {
                    Color::srgba(0.55, 0.34, 0.08, 0.90)
                } else if status_badge == "READY" || status_badge == "PLACE" {
                    Color::srgba(0.10, 0.50, 0.20, 0.92)
                } else {
                    Color::srgba(0.05, 0.14, 0.24, 0.88)
                };
                top_row
                    .spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(badge_bg),
                    ))
                    .with_children(|badge| {
                        badge.spawn((
                            Text::new(studio_layout::sanitize_ui_text(status_badge)),
                            TextFont {
                                font_size: 8.5,
                                ..default()
                            },
                            TextColor(Color::srgb(0.84, 0.98, 0.88)),
                        ));
                    });
            } else {
                top_row.spawn(Node::default());
            }

            if !cost_label.is_empty() {
                top_row
                    .spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(3.5), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.84)),
                    ))
                    .with_children(|cost_pill| {
                        cost_pill.spawn((
                            Text::new(studio_layout::sanitize_ui_text(cost_label)),
                            TextFont {
                                font_size: 9.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.90, 0.38)),
                        ));
                    });
            }
        });

        // Bottom strip: fallback text title ONLY when PNG cameo is absent + slim 3.5px bottom progress bar
        if icon_handle.is_none() || progress_pct.is_some() {
            btn.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            })
            .with_children(|bottom_wrap| {
                if icon_handle.is_none() {
                    bottom_wrap
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.82)),
                        ))
                        .with_children(|lbl_box| {
                            lbl_box.spawn((
                                Text::new(studio_layout::sanitize_ui_text(short_title)),
                                TextFont {
                                    font_size: 9.5,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                }

                if let Some(pct) = progress_pct {
                    bottom_wrap
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(3.5),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.86)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                Node {
                                    width: Val::Percent(pct.clamp(2.0, 100.0)),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.24, 0.95, 0.44)),
                            ));
                        });
                }
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_sidebar_cameo_grid(
    grid: &mut ChildBuilder,
    active_tab: ProductionSidebarTab,
    loaded: &LoadedMod,
    world: &SimWorld,
    interaction: &ClientInteractionState,
    pid: u8,
    base_buildings: &[&String],
    def_buildings: &[&String],
    inf_units: &[&String],
    veh_units: &[&String],
    ready_b_id: Option<&str>,
    building_queue_info: Option<&(String, f32, bool)>,
    unit_queue_counts: &BTreeMap<String, usize>,
    unit_head_progress: &BTreeMap<String, (u32, bool)>,
    cameo_snapshot: &HashMap<String, Option<Handle<Image>>>,
) {
    if active_tab == ProductionSidebarTab::Vehicles {
        grid.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|dom_row| {
            for dom in [
                VehicleDomainFilter::All,
                VehicleDomainFilter::Ground,
                VehicleDomainFilter::Naval,
                VehicleDomainFilter::Air,
            ] {
                let col = if interaction.vehicle_domain_filter == dom {
                    Color::srgb(0.20, 0.48, 0.68)
                } else {
                    Color::srgb(0.10, 0.16, 0.26)
                };
                spawn_styled_button(
                    dom_row,
                    dom.label(),
                    SidebarAction::SetVehicleDomainFilter(dom),
                    col,
                    Val::Percent(24.0),
                );
            }
        });
    }

    let preset = interaction.effective_hud_layout(loaded);
    let cameos_per_page = cameos_per_page_for_preset(preset);
    let btn_size = match preset {
        HudLayoutPreset::ClassicRightSidebar => (104.0, 68.0),
        HudLayoutPreset::ModernBottomCommandCard => (86.0, 48.0),
        HudLayoutPreset::CompactBarOverlay => (92.0, 50.0),
    };

    match active_tab {
        ProductionSidebarTab::Structures | ProductionSidebarTab::Defenses => {
            let list = if active_tab == ProductionSidebarTab::Structures {
                base_buildings
            } else {
                def_buildings
            };
            let total_pages = list.len().div_ceil(cameos_per_page).max(1);
            let page = interaction.sidebar_tab_pages[active_tab.index()].min(total_pages - 1);
            for (slot_idx, &b_id) in list
                .iter()
                .skip(page * cameos_per_page)
                .take(cameos_per_page)
                .enumerate()
            {
                if let Some(b_def) = loaded.buildings.get(b_id) {
                    let is_placing =
                        interaction.placing_building_id.as_deref() == Some(b_id.as_str());
                    let is_ready = ready_b_id == Some(b_id.as_str());
                    let queued_match = building_queue_info.filter(|(qid, _, _)| qid == b_id);
                    let hk_prefix = if interaction.grid_hotkeys_enabled
                        && slot_idx < GRID_CAMEO_KEY_LABELS.len()
                    {
                        format!("[{}] ", GRID_CAMEO_KEY_LABELS[slot_idx])
                    } else {
                        String::new()
                    };
                    let (status_tag, progress_pct) = if is_placing {
                        (format!("{hk_prefix}PLACE"), Some(100.0))
                    } else if is_ready {
                        (format!("{hk_prefix}READY"), Some(100.0))
                    } else if let Some((_, pct, paused)) = queued_match {
                        if *paused {
                            (format!("{hk_prefix}HOLD {:.0}%", pct), Some(*pct))
                        } else {
                            (format!("{hk_prefix}{:.0}%", pct), Some(*pct))
                        }
                    } else if !hk_prefix.is_empty() {
                        (hk_prefix.trim().to_string(), None)
                    } else {
                        (String::new(), None)
                    };
                    let reqs = format_prereqs(loaded, &b_def.prerequisites, "Construction Yard");
                    let tip = format!(
                        "{}  [${} | {:.0}s | Power {:+} | {}x{}]\nRequires: {}  |  LMB: Queue / Place  |  RMB: Hold / Cancel",
                        b_def.name, b_def.cost, b_def.build_time_secs, b_def.power_delta, b_def.footprint[0], b_def.footprint[1], reqs
                    );
                    let title_str = truncate_cameo_title(&b_def.name, 14);
                    spawn_cameo_grid_button_sized(
                        grid,
                        btn_size,
                        &title_str,
                        &format!("${}", b_def.cost),
                        &status_tag,
                        cameo_snapshot.get(b_id).cloned().flatten(),
                        SidebarAction::StartPlaceBuilding(b_id.clone()),
                        if is_placing || is_ready {
                            Color::srgb(0.22, 0.48, 0.28)
                        } else if queued_match.is_some() {
                            Color::srgb(0.32, 0.40, 0.18)
                        } else {
                            Color::srgb(0.14, 0.24, 0.18)
                        },
                        is_placing || is_ready,
                        progress_pct,
                        Some(tip),
                    );
                }
            }
            if total_pages > 1 {
                super::sidebar_drawers::spawn_cameo_pagination_row(grid, page, total_pages);
            }
        }
        ProductionSidebarTab::Infantry | ProductionSidebarTab::Vehicles => {
            let raw_list = if active_tab == ProductionSidebarTab::Infantry {
                inf_units
            } else {
                veh_units
            };
            let filtered: Vec<&String> = raw_list
                .iter()
                .copied()
                .filter(|u_id| {
                    active_tab != ProductionSidebarTab::Vehicles
                        || loaded
                            .units
                            .get(*u_id)
                            .is_some_and(|u| interaction.vehicle_domain_filter.matches(u))
                })
                .collect();
            let total_pages = filtered.len().div_ceil(cameos_per_page).max(1);
            let page = interaction.sidebar_tab_pages[active_tab.index()].min(total_pages - 1);
            for (slot_idx, &u_id) in filtered
                .iter()
                .skip(page * cameos_per_page)
                .take(cameos_per_page)
                .enumerate()
            {
                if let Some(u_def) = loaded.units.get(u_id) {
                    let q_cnt = unit_queue_counts.get(u_id).copied().unwrap_or(0);
                    let hk_prefix = if interaction.grid_hotkeys_enabled
                        && slot_idx < GRID_CAMEO_KEY_LABELS.len()
                    {
                        format!("[{}] ", GRID_CAMEO_KEY_LABELS[slot_idx])
                    } else {
                        String::new()
                    };
                    let (status_tag, progress_pct) = if q_cnt > 0 {
                        if let Some(&(pct, paused)) = unit_head_progress.get(u_id) {
                            if paused {
                                (format!("{hk_prefix}HOLD x{q_cnt}"), Some(pct as f32))
                            } else {
                                (format!("{hk_prefix}x{q_cnt} {pct}%"), Some(pct as f32))
                            }
                        } else {
                            (format!("{hk_prefix}x{q_cnt}"), None)
                        }
                    } else if !hk_prefix.is_empty() {
                        (hk_prefix.trim().to_string(), None)
                    } else {
                        (String::new(), None)
                    };
                    let eff_cost = world.effective_unit_cost(pid, u_def, loaded);
                    let reqs = format_prereqs(loaded, &u_def.prerequisites, "Production Factory");
                    let tip = format!(
                        "{}  [${eff_cost} | {:.0}s | {:.0} HP | {:?}/{:?}]\nRequires: {reqs}  |  LMB: Train (Shift+LMB: x5)  |  Ctrl+RMB: On Hold  |  RMB: Cancel",
                        u_def.name, u_def.build_time_secs, u_def.health, u_def.role, u_def.movement_domain
                    );
                    let title_str = truncate_cameo_title(&u_def.name, 14);
                    spawn_cameo_grid_button_sized(
                        grid,
                        btn_size,
                        &title_str,
                        &format!("${eff_cost}"),
                        &status_tag,
                        cameo_snapshot.get(u_id).cloned().flatten(),
                        SidebarAction::ProduceUnit(u_id.clone()),
                        if q_cnt > 0 {
                            Color::srgb(0.22, 0.38, 0.54)
                        } else {
                            Color::srgb(0.15, 0.23, 0.36)
                        },
                        q_cnt > 0,
                        progress_pct,
                        Some(tip),
                    );
                }
            }
            if total_pages > 1 {
                super::sidebar_drawers::spawn_cameo_pagination_row(grid, page, total_pages);
            }
        }
    }
}
