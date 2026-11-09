use super::*;
use crate::input::{ClientInteractionState, ProductionSidebarTab};
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_asset_pipeline::discovery;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingDef, BuildingRole, UnitDef, UnitRole};
use rts_simulation::world::SimWorld;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
    container_ent: Entity,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    pid: u8,
    has_next_campaign_mission: bool,
    _selected_subgroups: &BTreeMap<String, usize>,
    avail_support_powers: &[(rts_sdk::schema::SupportPowerDef, f32)],
    avail_buildings: &[String],
    avail_units: &[String],
    avail_upgrades: &[String],
    unit_queue_counts: &BTreeMap<String, usize>,
) {
    let ready_b_id = world.players.get(&pid).and_then(|e| e.ready_building_id.clone());
    let queued_b_id = world
        .players
        .get(&pid)
        .and_then(|e| e.building_queue.as_ref().map(|(id, _, _)| id.clone()));
    let promo_pts = world
        .players
        .get(&pid)
        .map(|e| e.promotion_points)
        .unwrap_or(0);
    let active_faction_id = world
        .players
        .get(&pid)
        .map(|e| e.faction.as_str())
        .unwrap_or("allies");
    let hud_skin_opt = crate::input::resolve_active_faction_hud_skin(loaded, active_faction_id);
    let side1_img = hud_skin_opt
        .and_then(|s| s.side_header.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let repair_img = hud_skin_opt
        .and_then(|s| s.repair_button.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let sell_img = hud_skin_opt
        .and_then(|s| s.sell_button.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let side2_img = hud_skin_opt
        .and_then(|s| s.cameo_row_frame.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let side3_img = hud_skin_opt
        .and_then(|s| s.cameo_footer_frame.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let addon_img = hud_skin_opt
        .and_then(|s| s.addon_panel.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let power_icon_img = hud_skin_opt
        .and_then(|s| s.power_bar_icon.as_deref())
        .and_then(|p| {
            super::menu_screens::load_ui_image(
                asset_server,
                &crate::input::qualify_mod_asset_rel_path(loaded, p),
            )
        });
    let tab_icon_handles: Vec<Option<Handle<Image>>> = (0..4)
        .map(|idx| {
            hud_skin_opt
                .and_then(|s| s.tab_icons.get(idx))
                .and_then(|p| {
                    super::menu_screens::load_ui_image(
                        asset_server,
                        &crate::input::qualify_mod_asset_rel_path(loaded, p),
                    )
                })
        })
        .collect();
    let (pwr_prod, pwr_cons, has_power) = world
        .players
        .get(&pid)
        .map(|e| (e.power_produced, e.power_consumed, !e.is_low_power()))
        .unwrap_or((100, 50, true));

    commands.entity(container_ent).with_children(|parent| {
        if has_next_campaign_mission {
            spawn_styled_button(
                parent,
                "★ NEXT CAMPAIGN MISSION ▶",
                SidebarAction::AdvanceNextCampaignMission,
                Color::srgb(0.18, 0.56, 0.28),
                Val::Percent(100.0),
            );
        }

        // Split available structures and units into the 4 classic C&C/RA2 Production Sidebar Tabs
        let mut base_buildings: Vec<&String> = Vec::new();
        let mut def_buildings: Vec<&String> = Vec::new();
        for b_id in avail_buildings {
            if let Some(b_def) = loaded.buildings.get(b_id) {
                if is_defense_or_support_structure(b_def) {
                    def_buildings.push(b_id);
                } else {
                    base_buildings.push(b_id);
                }
            }
        }
        let mut inf_units: Vec<&String> = Vec::new();
        let mut veh_units: Vec<&String> = Vec::new();
        for u_id in avail_units {
            if let Some(u_def) = loaded.units.get(u_id) {
                if is_infantry_unit(u_def) {
                    inf_units.push(u_id);
                } else {
                    veh_units.push(u_id);
                }
            }
        }

        // 1. Authentic RA2 `side1.png` Command & Category Tab Deck (Repair/Sell Arch + 4 Glowing Tab Slots)
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
                // Top Arch Row: Graphical REPAIR (`repair.png`) & SELL (`sell.png`) Buttons
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
                        for (btn_img, fallback_lbl, act) in [
                            (
                                repair_img.clone(),
                                "REPAIR [R]",
                                SidebarAction::RepairSelectedBuildings,
                            ),
                            (
                                sell_img.clone(),
                                "SELL [$]",
                                SidebarAction::SellSelectedBuildings,
                            ),
                        ] {
                            let has_icon = btn_img.is_some();
                            arch_row
                                .spawn((
                                    Button,
                                    act,
                                    ButtonVisualStyle {
                                        normal: Color::WHITE,
                                        hovered: Color::srgb(0.86, 0.96, 1.0),
                                        pressed: Color::srgb(0.66, 0.78, 0.94),
                                    },
                                    Node {
                                        width: Val::Px(104.0),
                                        height: Val::Px(34.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(if has_icon { 0.0 } else { 1.0 })),
                                        ..default()
                                    },
                                    BackgroundColor(if has_icon {
                                        Color::WHITE
                                    } else {
                                        Color::srgb(0.18, 0.34, 0.52)
                                    }),
                                    BorderColor(Color::srgba(0.65, 0.85, 1.0, 0.45)),
                                    btn_img.map(ImageNode::new).unwrap_or_default(),
                                ))
                                .with_children(|b| {
                                    if !has_icon {
                                        b.spawn((
                                            Node {
                                                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(0.02, 0.05, 0.12, 0.65)),
                                        ))
                                        .with_children(|tag| {
                                            tag.spawn((
                                                Text::new(fallback_lbl),
                                                TextFont {
                                                    font_size: 9.0,
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.95, 0.98, 1.0)),
                                            ));
                                        });
                                    }
                                });
                        }
                    });

                // Bottom Row: 4 Glowing Category Tab Slots (`tab00.png` Building, `tab01.png` Shield, `tab02.png` Infantry, `tab03.png` Tank)
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
                                tab_icon_handles.first().cloned().flatten(),
                            ),
                            (
                                ProductionSidebarTab::Defenses,
                                format!("DEF ({})", def_buildings.len()),
                                tab_icon_handles.get(1).cloned().flatten(),
                            ),
                            (
                                ProductionSidebarTab::Infantry,
                                format!("INF ({})", inf_units.len()),
                                tab_icon_handles.get(2).cloned().flatten(),
                            ),
                            (
                                ProductionSidebarTab::Vehicles,
                                format!("VEH ({})", veh_units.len()),
                                tab_icon_handles.get(3).cloned().flatten(),
                            ),
                        ];
                        for (tab_val, tab_lbl, tab_icon) in tab_specs {
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
                                ButtonVisualStyle {
                                    normal: normal_c,
                                    hovered: Color::WHITE,
                                    pressed: Color::srgb(0.75, 0.85, 1.0),
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
                                        Node {
                                            padding: UiRect::axes(Val::Px(2.5), Val::Px(0.5)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.02, 0.04, 0.10, 0.78)),
                                    ))
                                    .with_children(|lbl| {
                                        lbl.spawn((
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
                                    });
                                }
                            });
                        }
                    });
            });

        // 2. Authentic RA2 `side2.png` Area: Left Vertical Power Bar (`power.png`) + Right 2-Column Full-Bleed Cameo Slots
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                ..default()
            })
            .with_children(|middle_deck| {
                // 2.1 Left Vertical Power Bar Channel (`22px` wide)
                let pwr_ratio: f32 = if pwr_prod <= 0 {
                    0.08
                } else {
                    let net_ratio = ((pwr_prod.saturating_sub(pwr_cons)) as f32
                        / pwr_prod.max(1) as f32)
                        .clamp(0.0_f32, 1.0_f32);
                    (0.18 + net_ratio * 0.82).clamp(0.10_f32, 1.0_f32)
                };
                let pwr_bar_col = if !has_power {
                    Color::srgb(0.96, 0.20, 0.18)
                } else if pwr_prod.saturating_sub(pwr_cons) < 40 {
                    Color::srgb(0.98, 0.82, 0.20)
                } else {
                    Color::srgb(0.20, 0.95, 0.38)
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
                                    height: Val::Px(280.0),
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

                // 2.2 Right 2-Column Full-Bleed Cameo Bezel Container (`246px` wide, framed by `side2.png`)
                middle_deck
                    .spawn((
                        Node {
                            width: Val::Px(246.0),
                            min_height: Val::Px(396.0),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::SpaceEvenly,
                            align_content: AlignContent::FlexStart,
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                            column_gap: Val::Px(6.0),
                            row_gap: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(if side2_img.is_some() {
                            Color::WHITE
                        } else {
                            Color::srgb(0.06, 0.09, 0.14)
                        }),
                        side2_img.map(ImageNode::new).unwrap_or_default(),
                    ))
                    .with_children(|grid| {
                        match active_tab {
                            ProductionSidebarTab::Structures | ProductionSidebarTab::Defenses => {
                                let list = if active_tab == ProductionSidebarTab::Structures {
                                    &base_buildings
                                } else {
                                    &def_buildings
                                };
                                for &b_id in list.iter().take(10) {
                                    if let Some(b_def) = loaded.buildings.get(b_id) {
                                        let is_placing = interaction.placing_building_id.as_deref()
                                            == Some(b_id.as_str());
                                        let is_ready = ready_b_id.as_deref() == Some(b_id.as_str());
                                        let is_queued =
                                            queued_b_id.as_deref() == Some(b_id.as_str());
                                        let status_tag = if is_placing {
                                            "PLACE"
                                        } else if is_ready {
                                            "READY"
                                        } else if is_queued {
                                            "BLDG"
                                        } else {
                                            ""
                                        };
                                        let short_title = truncate_cameo_title(&b_def.name, 14);
                                        let cost_label = format!("${}", b_def.cost);
                                        let color = if is_placing || is_ready {
                                            Color::srgb(0.22, 0.48, 0.28)
                                        } else if is_queued {
                                            Color::srgb(0.32, 0.40, 0.18)
                                        } else {
                                            Color::srgb(0.14, 0.24, 0.18)
                                        };
                                        let cameo = super::menu_screens::load_mod_cameo(
                                            asset_server,
                                            loaded,
                                            b_id,
                                        );
                                        studio_layout::spawn_cameo_grid_button(
                                            grid,
                                            &short_title,
                                            &cost_label,
                                            status_tag,
                                            cameo,
                                            SidebarAction::StartPlaceBuilding(b_id.clone()),
                                            color,
                                            is_placing || is_ready,
                                        );
                                    }
                                }
                            }
                            ProductionSidebarTab::Infantry | ProductionSidebarTab::Vehicles => {
                                let list = if active_tab == ProductionSidebarTab::Infantry {
                                    &inf_units
                                } else {
                                    &veh_units
                                };
                                for &u_id in list.iter().take(10) {
                                    if let Some(u_def) = loaded.units.get(u_id) {
                                        let q_cnt =
                                            unit_queue_counts.get(u_id).copied().unwrap_or(0);
                                        let status_tag = if q_cnt > 0 {
                                            format!("x{q_cnt}")
                                        } else {
                                            String::new()
                                        };
                                        let short_title = truncate_cameo_title(&u_def.name, 14);
                                        let cost_label = format!("${}", u_def.cost);
                                        let btn_col = if q_cnt > 0 {
                                            Color::srgb(0.22, 0.38, 0.54)
                                        } else {
                                            Color::srgb(0.15, 0.23, 0.36)
                                        };
                                        let cameo = super::menu_screens::load_mod_cameo(
                                            asset_server,
                                            loaded,
                                            u_id,
                                        );
                                        studio_layout::spawn_cameo_grid_button(
                                            grid,
                                            &short_title,
                                            &cost_label,
                                            &status_tag,
                                            cameo,
                                            SidebarAction::ProduceUnit(u_id.clone()),
                                            btn_col,
                                            q_cnt > 0,
                                        );
                                    }
                                }
                            }
                        }
                    });
            });

        // 3. Authentic RA2 `side3.png` Curved Emblem Footer (`34px` height) + `addon.png` Lower Bezel
        parent.spawn((
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
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(42.0),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                ImageNode::new(a_img),
            ));
        }

        // 4. Compact Support Powers & Doctrines Drawer (below `side3.png` when unlocked)
        if !avail_support_powers.is_empty() {
            for (sp_def, rem_secs) in avail_support_powers.iter().take(2) {
                let is_targeting =
                    interaction.active_support_power_id.as_deref() == Some(sp_def.id.as_str());
                let is_ready = *rem_secs <= 0.0;
                let lbl = if is_targeting {
                    format!(">> SELECT TARGET: {} <<", sp_def.name)
                } else if is_ready {
                    format!("★ [READY] {}", sp_def.name)
                } else {
                    format!("[{}s] {}", rem_secs.ceil() as u32, sp_def.name)
                };
                let col = if is_targeting || is_ready {
                    Color::srgb(0.48, 0.34, 0.12)
                } else {
                    Color::srgb(0.20, 0.22, 0.28)
                };
                spawn_styled_button(
                    parent,
                    &lbl,
                    SidebarAction::TriggerSupportPower(sp_def.id.clone()),
                    col,
                    Val::Percent(100.0),
                );
            }
        }

        if !avail_upgrades.is_empty() && promo_pts > 0 {
            if let Some(up_id) = avail_upgrades.first() {
                if let Some(up_def) = loaded.upgrades.get(up_id) {
                    let lbl = format!("★ [DOCTRINE] {}", up_def.name);
                    spawn_styled_button(
                        parent,
                        &lbl,
                        SidebarAction::ResearchUpgrade(up_id.clone()),
                        Color::srgb(0.44, 0.32, 0.14),
                        Val::Percent(100.0),
                    );
                }
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_studio_modal_list(
    commands: &mut Commands,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    last_sig: &mut LastSidebarSignature,
    studio_mods_q: &Query<Entity, With<StudioModListContainer>>,
    pid: u8,
) {
    let discovered_mods = discovery::discover_available_mods(&asset_root.0);
    let map_keys: Vec<String> = loaded.maps.keys().cloned().collect();
    let mut save_files: Vec<PathBuf> = Vec::new();
    let mut replay_files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir("saves") {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if name.ends_with(".replay.json") {
                replay_files.push(p);
            } else if name.ends_with(".json") {
                save_files.push(p);
            }
        }
    }
    save_files.sort();
    replay_files.sort();

    let lobbies_sig: String = interaction
        .online_lobbies
        .iter()
        .map(|l| {
            format!(
                "{}:{}:{}/{}",
                l.lobby_id, l.host_addr, l.current_players, l.max_players
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let load_step = if interaction.app_phase == crate::input::AppPhase::LoadingScreen {
        (interaction.loading_progress * 20.0).round() as i32
    } else {
        0
    };
    let studio_sig = format!(
        "dev={}:phase={:?}:load={load_step}:screen={:?}:cfac={}:efac={}:eent={:?}:ctry={}:ai={}:{:?}:{:?}:fog={}:env={:?}:srv={}:active={}:map={}:camp={:?}:{:?}:cpage={}:mpage={}:mall={}:seed={}:vid={:?}:ip={}:{}:lobbies={}:mods={}:maps={}:saves={}:replays={}",
        interaction.dev_mode_enabled,
        interaction.app_phase,
        interaction.active_menu_screen,
        interaction.selected_campaign_faction,
        interaction.selected_encyclopedia_faction,
        interaction.selected_encyclopedia_entry,
        interaction.skirmish_country_index,
        interaction.ai_enabled,
        interaction.ai_difficulty,
        interaction.ai_personality,
        interaction.fog_of_war_enabled,
        interaction.environment_preset,
        interaction.lobby_server_url,
        loaded.manifest.id,
        interaction.active_map_id,
        interaction.active_campaign_id,
        interaction.active_mission_id,
        interaction.campaign_mission_page,
        interaction.skirmish_map_page,
        interaction.skirmish_show_all_maps,
        interaction.procedural_map_seed,
        interaction.active_video_preview_path,
        interaction.server_addr_input,
        interaction.ip_input_editing,
        lobbies_sig,
        discovered_mods
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";"),
        map_keys.join(","),
        save_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";"),
        replay_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";")
    );

    if studio_sig != last_sig.studio_sig {
        last_sig.studio_sig = studio_sig;
        if let Ok(mod_list_ent) = studio_mods_q.get_single() {
            commands.entity(mod_list_ent).despawn_descendants();
            commands.entity(mod_list_ent).with_children(|parent| {
                super::menu_screens::populate_active_menu_screen(
                    parent,
                    asset_server,
                    asset_root,
                    loaded,
                    interaction,
                    &discovered_mods,
                    &save_files,
                    &replay_files,
                    pid,
                );
            });
        }
    }
}
