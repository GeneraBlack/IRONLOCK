use super::*;
use crate::input::{ClientInteractionState, ProductionSidebarTab, UiAssetCache};
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingDef, BuildingRole, UnitDef, UnitRole};
use rts_simulation::world::SimWorld;
use std::collections::BTreeMap;

fn is_defense_or_support_structure(b_def: &BuildingDef) -> bool {
    matches!(b_def.role, BuildingRole::Defense | BuildingRole::Wall)
        || b_def.primary_weapon.is_some()
        || b_def.superweapon.is_some()
        || !b_def.support_powers.is_empty()
        || b_def.stealth.cloak_field_radius > 0.0
        || b_def.stealth.psychic_sensor_radius > 0.0
}

fn is_infantry_unit(u_def: &UnitDef) -> bool {
    matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer)
        || u_def.movement_domain == rts_sdk::schema::MovementDomain::Jumpjet
}

#[allow(clippy::too_many_arguments)]
pub(super) fn populate_sidebar_buttons(
    commands: &mut Commands,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    container_ent: Entity,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    pid: u8,
    has_next_campaign_mission: bool,
    worker_buildable_ids: &[String],
    avail_support_powers: &[(rts_sdk::schema::SupportPowerDef, f32)],
    avail_buildings: &[String],
    avail_units: &[String],
    avail_upgrades: &[String],
    unit_queue_counts: &BTreeMap<String, usize>,
    unit_head_progress: &BTreeMap<String, (u32, bool)>,
) {
    let econ_opt = world.players.get(&pid);
    let ready_b_id = econ_opt.and_then(|e| e.ready_building_id.clone());
    let building_queue_info = econ_opt.and_then(|e| {
        e.building_queue.as_ref().map(|(id, rem, total)| {
            let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(1.0, 99.0);
            (id.clone(), pct, e.building_queue_paused)
        })
    });
    let active_upgrade_info = econ_opt.and_then(|e| {
        e.active_upgrade.as_ref().map(|(id, rem, total)| {
            let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(1.0, 99.0);
            (id.clone(), pct)
        })
    });
    let promo_pts = econ_opt.map(|e| e.promotion_points).unwrap_or(0);
    let active_faction_id = econ_opt.map(|e| e.faction.as_str()).unwrap_or("allies");
    let hud_skin_opt = crate::input::resolve_active_faction_hud_skin(loaded, active_faction_id);

    let side1_img = hud_skin_opt
        .and_then(|s| s.side_header.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let repair_img = hud_skin_opt
        .and_then(|s| s.repair_button.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let sell_img = hud_skin_opt
        .and_then(|s| s.sell_button.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let side2_img = hud_skin_opt
        .and_then(|s| s.cameo_row_frame.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let side3_img = hud_skin_opt
        .and_then(|s| s.cameo_footer_frame.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let addon_img = hud_skin_opt
        .and_then(|s| s.addon_panel.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let power_icon_img = hud_skin_opt
        .and_then(|s| s.power_bar_icon.as_deref())
        .and_then(|p| ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p));
    let tab_icon_handles: Vec<Option<Handle<Image>>> = (0..4)
        .map(|idx| {
            hud_skin_opt
                .and_then(|s| s.tab_icons.get(idx))
                .and_then(|p| {
                    ui_cache.load_qualified_mod_image(loaded, asset_server, &asset_root.0, p)
                })
        })
        .collect();

    let (pwr_prod, pwr_cons, has_power) = econ_opt
        .map(|e| (e.power_produced, e.power_consumed, !e.is_low_power()))
        .unwrap_or((100, 50, true));

    let mut base_buildings: Vec<&String> = Vec::new();
    let mut def_buildings: Vec<&String> = Vec::new();
    for b_id in avail_buildings {
        if let Some(b_def) = loaded.buildings.get(b_id) {
            let _ = ui_cache.load_entity_cameo(loaded, asset_server, b_id);
            if is_defense_or_support_structure(b_def) {
                def_buildings.push(b_id);
            } else {
                base_buildings.push(b_id);
            }
        }
    }
    for b_id in worker_buildable_ids {
        let _ = ui_cache.load_entity_cameo(loaded, asset_server, b_id);
    }
    let mut inf_units: Vec<&String> = Vec::new();
    let mut veh_units: Vec<&String> = Vec::new();
    for u_id in avail_units {
        if let Some(u_def) = loaded.units.get(u_id) {
            let _ = ui_cache.load_entity_cameo(loaded, asset_server, u_id);
            if is_infantry_unit(u_def) {
                inf_units.push(u_id);
            } else {
                veh_units.push(u_id);
            }
        }
    }
    let cameo_snapshot = ui_cache.entity_cameos.clone();

    commands.entity(container_ent).with_children(|parent| {
        if has_next_campaign_mission {
            spawn_styled_button(
                parent,
                ">> NEXT CAMPAIGN MISSION >>",
                SidebarAction::AdvanceNextCampaignMission,
                Color::srgb(0.18, 0.56, 0.28),
                Val::Percent(100.0),
            );
        }

        // Worker / Dozer On-Site Construction Drawer (whenever `BuilderTraitSpec` worker(s) are selected)
        if !worker_buildable_ids.is_empty() {
            super::sidebar_drawers::spawn_worker_builder_drawer(
                parent,
                loaded,
                interaction,
                worker_buildable_ids,
                &cameo_snapshot,
            );
        }

        // 1. Authentic RA2 `side1.png` Command & Category Tab Deck
        let active_tab = interaction.active_sidebar_tab;
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(94.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(if side1_img.is_some() {
                    Color::WHITE
                } else {
                    Color::srgb(0.12, 0.16, 0.24)
                }),
                side1_img.map(ImageNode::new).unwrap_or_default(),
            ))
            .with_children(|side1| {
                side1
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(36.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceEvenly,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|arch_row| {
                        for (btn_img, fallback_lbl, tip, act, armed, btn_w) in [
                            (
                                repair_img.clone(),
                                "REPAIR [R]",
                                "Repair Structure [R]\nClick a damaged friendly structure or press [R] while selected.",
                                SidebarAction::ToggleRepairCursor,
                                interaction.repair_cursor_mode,
                                78.0,
                            ),
                            (
                                sell_img.clone(),
                                "SELL [DEL]",
                                "Sell Structure [Delete]\nClick an owned structure or press [Delete] for 50% credit refund.",
                                SidebarAction::ToggleSellCursor,
                                interaction.sell_cursor_mode,
                                78.0,
                            ),
                            (
                                None,
                                "PING [ALT]",
                                "Tactical Radar Ping [Alt+Click]\nClick battlefield or minimap to broadcast an allied flare beacon.",
                                SidebarAction::TogglePingCursor,
                                interaction.ping_cursor_mode,
                                68.0,
                            ),
                        ] {
                            let has_icon = btn_img.is_some();
                            let arch_border = if armed {
                                Color::srgb(1.0, 0.88, 0.28)
                            } else {
                                Color::srgba(0.65, 0.85, 1.0, 0.45)
                            };
                            arch_row
                                .spawn((
                                    Button,
                                    act,
                                    CameoTooltip(tip.to_string()),
                                    ButtonVisualStyle {
                                        normal: if has_icon {
                                            Color::WHITE
                                        } else if armed {
                                            Color::srgb(0.32, 0.52, 0.28)
                                        } else {
                                            Color::srgb(0.18, 0.34, 0.52)
                                        },
                                        hovered: Color::srgb(0.86, 0.96, 1.0),
                                        pressed: Color::srgb(0.66, 0.78, 0.94),
                                        default_border: arch_border,
                                    },
                                    Node {
                                        width: Val::Px(btn_w),
                                        height: Val::Px(34.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(if armed {
                                            2.0
                                        } else if has_icon {
                                            0.0
                                        } else {
                                            1.0
                                        })),
                                        ..default()
                                    },
                                    BackgroundColor(if has_icon {
                                        Color::WHITE
                                    } else {
                                        Color::srgb(0.18, 0.34, 0.52)
                                    }),
                                    BorderColor(arch_border),
                                    btn_img.map(ImageNode::new).unwrap_or_default(),
                                ))
                                .with_children(|b| {
                                    if !has_icon {
                                        b.spawn((
                                            Text::new(fallback_lbl),
                                            TextFont {
                                                font_size: 9.0,
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.95, 0.98, 1.0)),
                                        ));
                                    }
                                });
                        }
                    });

                side1
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(42.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceEvenly,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|tabs| {
                        let tab_specs = [
                            (
                                ProductionSidebarTab::Structures,
                                format!("STR ({})", base_buildings.len()),
                                "Structures Tab: Base economy, power, refineries, and factories.",
                                tab_icon_handles.first().cloned().flatten(),
                            ),
                            (
                                ProductionSidebarTab::Defenses,
                                format!("DEF ({})", def_buildings.len()),
                                "Defenses Tab: Turrets, walls, radar, and superweapons.",
                                tab_icon_handles.get(1).cloned().flatten(),
                            ),
                            (
                                ProductionSidebarTab::Infantry,
                                format!("INF ({})", inf_units.len()),
                                "Infantry Tab: Squads, engineers, spies, and commandos.",
                                tab_icon_handles.get(2).cloned().flatten(),
                            ),
                            (
                                ProductionSidebarTab::Vehicles,
                                format!("VEH ({})", veh_units.len()),
                                "War Machines Tab: Ground armor, naval fleet, and aircraft.",
                                tab_icon_handles.get(3).cloned().flatten(),
                            ),
                        ];
                        for (tab_val, tab_lbl, tab_tip, tab_icon) in tab_specs {
                            let is_active = active_tab == tab_val;
                            let has_tab_img = tab_icon.is_some();
                            let border_c = if is_active {
                                Color::srgb(1.0, 0.90, 0.32)
                            } else {
                                Color::srgba(0.55, 0.78, 1.0, 0.32)
                            };
                            let normal_c = if has_tab_img {
                                if is_active {
                                    Color::WHITE
                                } else {
                                    Color::srgb(0.76, 0.84, 0.94)
                                }
                            } else if is_active {
                                Color::srgb(0.18, 0.48, 0.76)
                            } else {
                                Color::srgb(0.10, 0.22, 0.38)
                            };
                            tabs.spawn((
                                Button,
                                SidebarAction::SelectProductionTab(tab_val),
                                CameoTooltip(tab_tip.to_string()),
                                ButtonVisualStyle {
                                    normal: normal_c,
                                    hovered: Color::WHITE,
                                    pressed: Color::srgb(0.75, 0.85, 1.0),
                                    default_border: border_c,
                                },
                                Node {
                                    width: Val::Px(54.0),
                                    height: Val::Px(38.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::FlexEnd,
                                    padding: UiRect::bottom(Val::Px(2.0)),
                                    border: UiRect::all(Val::Px(if is_active { 2.0 } else { 0.5 })),
                                    ..default()
                                },
                                BackgroundColor(normal_c),
                                BorderColor(border_c),
                                tab_icon.map(ImageNode::new).unwrap_or_default(),
                            ))
                            .with_children(|tb| {
                                if !has_tab_img {
                                    tb.spawn((
                                        Text::new(studio_layout::sanitize_ui_text(&tab_lbl)),
                                        TextFont {
                                            font_size: 8.5,
                                            ..default()
                                        },
                                        TextColor(if is_active {
                                            Color::srgb(1.0, 0.92, 0.38)
                                        } else {
                                            Color::srgb(0.88, 0.94, 1.0)
                                        }),
                                    ));
                                }
                            });
                        }
                    });
            });

        // 2. Authentic RA2 `side2.png` Area: Left Vertical Power Bar + Right 2-Column Cameo Slots
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                ..default()
            })
            .with_children(|middle_deck| {
                let pwr_ratio: f32 = if pwr_prod <= 0 {
                    0.08
                } else {
                    let net_ratio = ((pwr_prod.saturating_sub(pwr_cons)) as f32
                        / pwr_prod.max(1) as f32)
                        .clamp(0.0, 1.0);
                    (0.18 + net_ratio * 0.82).clamp(0.10, 1.0)
                };
                let pwr_bar_col = if !has_power {
                    Color::srgb(0.96, 0.20, 0.18)
                } else if pwr_prod.saturating_sub(pwr_cons) < 40 {
                    Color::srgb(0.98, 0.82, 0.20)
                } else {
                    Color::srgb(0.20, 0.95, 0.38)
                };

                let hud_preset = interaction.effective_hud_layout(loaded);
                let (grid_w, grid_min_h, pwr_track_h) = match hud_preset {
                    rts_sdk::manifest::HudLayoutPreset::ClassicRightSidebar => {
                        (246.0, 396.0, 280.0)
                    }
                    rts_sdk::manifest::HudLayoutPreset::ModernBottomCommandCard => {
                        (380.0, 174.0, 152.0)
                    }
                    rts_sdk::manifest::HudLayoutPreset::CompactBarOverlay => {
                        (204.0, 218.0, 188.0)
                    }
                };
                middle_deck
                    .spawn((
                        Node {
                            width: Val::Px(22.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(2.0), Val::Px(4.0)),
                            row_gap: Val::Px(3.0),
                            border: UiRect::right(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.05, 0.08, 0.13)),
                        BorderColor(Color::srgb(0.22, 0.34, 0.50)),
                    ))
                    .with_children(|pwr_col| {
                        if let Some(p_img) = power_icon_img.clone() {
                            pwr_col.spawn((
                                ImageNode::new(p_img),
                                Node {
                                    width: Val::Px(16.0),
                                    height: Val::Px(18.0),
                                    ..default()
                                },
                            ));
                        }
                        pwr_col
                            .spawn((
                                Node {
                                    width: Val::Px(10.0),
                                    height: Val::Px(pwr_track_h),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::FlexEnd,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.08, 0.10, 0.14)),
                                BorderColor(Color::srgb(0.32, 0.46, 0.64)),
                            ))
                            .with_children(|track| {
                                track.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent((pwr_ratio * 100.0).round()),
                                        ..default()
                                    },
                                    BackgroundColor(pwr_bar_col),
                                ));
                            });
                    });

                middle_deck
                    .spawn((
                        Node {
                            width: Val::Px(grid_w),
                            min_height: Val::Px(grid_min_h),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::SpaceEvenly,
                            align_content: AlignContent::FlexStart,
                            padding: UiRect::axes(Val::Px(5.0), Val::Px(4.0)),
                            column_gap: Val::Px(5.0),
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(if side2_img.is_some()
                            && hud_preset == rts_sdk::manifest::HudLayoutPreset::ClassicRightSidebar
                        {
                            Color::WHITE
                        } else {
                            Color::srgb(0.06, 0.09, 0.14)
                        }),
                        if hud_preset == rts_sdk::manifest::HudLayoutPreset::ClassicRightSidebar {
                            side2_img.map(ImageNode::new).unwrap_or_default()
                        } else {
                            ImageNode::default()
                        },
                    ))
                    .with_children(|grid| {
                        super::sidebar_cameos::spawn_sidebar_cameo_grid(
                            grid,
                            active_tab,
                            loaded,
                            world,
                            interaction,
                            pid,
                            &base_buildings,
                            &def_buildings,
                            &inf_units,
                            &veh_units,
                            ready_b_id.as_deref(),
                            building_queue_info.as_ref(),
                            unit_queue_counts,
                            unit_head_progress,
                            &cameo_snapshot,
                        );
                    });
            });

        // 3. Authentic RA2 `side3.png` Curved Emblem Footer + `addon.png` + Powers/Upgrades Drawers
        if interaction.effective_hud_layout(loaded)
            == rts_sdk::manifest::HudLayoutPreset::ClassicRightSidebar
        {
            parent.spawn((
                super::hud_layouts::ClassicSidebarChromeOnly,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(34.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if side3_img.is_some() {
                    Color::WHITE
                } else {
                    Color::srgb(0.10, 0.14, 0.22)
                }),
                side3_img.map(ImageNode::new).unwrap_or_default(),
            ));
            if let Some(a_img) = addon_img {
                parent.spawn((
                    super::hud_layouts::ClassicSidebarChromeOnly,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(42.0),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                    ImageNode::new(a_img),
                ));
            }
        }

        super::sidebar_drawers::spawn_support_powers_and_upgrades_drawers(
            parent,
            loaded,
            interaction,
            promo_pts,
            avail_support_powers,
            avail_upgrades,
            active_upgrade_info.as_ref(),
        );
    });
}
