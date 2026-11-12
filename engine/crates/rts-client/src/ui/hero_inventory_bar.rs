use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::ui::locale::tr;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::{ButtonVisualStyle, CameoTooltip, HudLayoutRole, HudPanelBlocker, SidebarAction};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::manifest::HudLayoutPreset;
use rts_simulation::world::EntityKind;

#[derive(Component)]
pub struct HeroInventoryBarContent;

#[derive(Default)]
pub struct HeroInventorySigCache(pub String);

pub fn spawn_hero_inventory_bar(commands: &mut Commands) {
    commands
        .spawn((
            HudLayoutRole::HeroInventoryBarRoot,
            HudPanelBlocker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(240.0),
                bottom: Val::Px(8.0),
                display: Display::None,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.94)),
            BorderColor(Color::srgba(0.88, 0.72, 0.30, 0.88)),
            ZIndex(25),
        ))
        .with_children(|root| {
            root.spawn((
                HeroInventoryBarContent,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
            ));
        });
}

/// Renders the Hero Portrait, Level/XP, HP/Mana (`energy / max_energy`) bars, `[+]` Skill-Up
/// buttons (`LearnHeroSkill`), Altar `[Revive Hero]` button, and 6-Slot Inventory (`Task 240`).
/// In `ModernBottomCommandCard`, also renders the selected unit/structure portrait & vitals card!
#[allow(clippy::too_many_arguments)]
pub fn update_hero_inventory_bar(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    interaction: Res<ClientInteractionState>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    mut ui_cache: ResMut<UiAssetCache>,
    mut sig_cache: Local<HeroInventorySigCache>,
    mut layout_q: Query<(&mut Node, &HudLayoutRole)>,
    content_q: Query<Entity, With<HeroInventoryBarContent>>,
) {
    let show_hud = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let pid = interaction.local_player_id;
    let lang = interaction.language.as_str();
    let preset = interaction.effective_hud_layout(loaded);

    // 1. Find selected hero (prefer owned selected hero, fallback to first owned living hero in ModernBottomCommandCard)
    let mut selected_hero_id: Option<u64> = interaction
        .selected_ids
        .iter()
        .copied()
        .find(|id| world.trait_stores.heroes.contains_key(id));

    if selected_hero_id.is_none()
        && preset == HudLayoutPreset::ModernBottomCommandCard
        && interaction.selected_ids.is_empty()
    {
        selected_hero_id = world
            .entities
            .values()
            .find(|e| {
                e.player_id == pid
                    && e.health > 0.0
                    && world.trait_stores.heroes.contains_key(&e.id)
            })
            .map(|e| e.id);
    }

    let fallen_list = world
        .trait_stores
        .fallen_heroes
        .get(&pid)
        .cloned()
        .unwrap_or_default();

    let primary_selected_ent = interaction
        .selected_ids
        .first()
        .and_then(|id| world.entities.get(id));

    let show_bar = show_hud
        && (selected_hero_id.is_some()
            || !fallen_list.is_empty()
            || (preset == HudLayoutPreset::ModernBottomCommandCard
                && primary_selected_ent.is_some()));

    for (mut node, role) in &mut layout_q {
        if *role == HudLayoutRole::HeroInventoryBarRoot {
            node.display = if show_bar {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    if !show_bar {
        sig_cache.0.clear();
        return;
    }

    let hero_ent = selected_hero_id.and_then(|id| world.entities.get(&id));
    let hero_state = selected_hero_id.and_then(|id| world.trait_stores.heroes.get(&id));
    let energy_state = selected_hero_id
        .or_else(|| primary_selected_ent.map(|e| e.id))
        .and_then(|id| world.trait_stores.energy_pools.get(&id));

    let sig = format!(
        "p={preset:?}|lang={lang}|hid={selected_hero_id:?}|hp={:?}|xp={:?}|en={:?}|hs={hero_state:?}|fallen={}|sel={:?}",
        hero_ent.map(|e| (e.health as i32, e.max_health as i32)),
        hero_ent.map(|e| e.experience as i32),
        energy_state.map(|en| (en.energy as i32, en.max_energy as i32)),
        fallen_list
            .iter()
            .map(|f| format!("{}:{}", f.unit_id, f.hero_state.level))
            .collect::<Vec<_>>()
            .join(","),
        primary_selected_ent.map(|e| (e.id, e.health as i32, interaction.selected_ids.len()))
    );

    if sig == sig_cache.0 {
        return;
    }
    sig_cache.0 = sig;

    let Ok(content_ent) = content_q.get_single() else {
        return;
    };
    commands.entity(content_ent).despawn_descendants();

    // Preload portrait & item cameos before spawning children
    let portrait_entity_key: Option<String> =
        hero_ent
            .or(primary_selected_ent)
            .map(|ent| match &ent.kind {
                EntityKind::Unit { unit_id } => unit_id.clone(),
                EntityKind::Building { building_id } => building_id.clone(),
            });
    let portrait_img = portrait_entity_key
        .as_deref()
        .and_then(|key| ui_cache.load_entity_cameo(loaded, asset_server.as_deref(), key));

    let mut slot_icons: Vec<Option<Handle<Image>>> = Vec::new();
    if let Some(hs) = hero_state {
        for idx in 0..6 {
            let icon = hs
                .inventory
                .get(idx)
                .and_then(|s| s.as_ref())
                .and_then(|item| {
                    ui_cache.load_item_cameo(loaded, asset_server.as_deref(), &item.item_id)
                });
            slot_icons.push(icon);
        }
    }

    commands.entity(content_ent).with_children(|bar| {
        if let (Some(h_ent), Some(hs)) = (hero_ent, hero_state) {
            let EntityKind::Unit { ref unit_id } = h_ent.kind else {
                return;
            };
            let u_def = loaded.units.get(unit_id);
            let h_name = u_def.map(|u| u.name.as_str()).unwrap_or(unit_id.as_str());
            let hero_spec = u_def.and_then(|u| u.hero_trait.as_ref());

            // Next XP threshold
            let (cur_floor, next_xp) = if let Some(spec) = hero_spec {
                let idx = (hs.level as usize).saturating_sub(1);
                let floor = if idx == 0 {
                    0.0
                } else {
                    spec.xp_curve
                        .get(idx.saturating_sub(1))
                        .copied()
                        .unwrap_or(0.0)
                };
                let ceil = spec.xp_curve.get(idx).copied().unwrap_or(floor + 200.0);
                (floor, ceil.max(floor + 1.0))
            } else {
                (0.0, 200.0)
            };
            let xp_pct = (((h_ent.experience - cur_floor) / (next_xp - cur_floor)) * 100.0)
                .clamp(0.0, 100.0);
            let hp_pct = ((h_ent.health / h_ent.max_health.max(1.0)) * 100.0).clamp(0.0, 100.0);

            // Portrait + Level Badge
            bar.spawn((
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::FlexEnd,
                    padding: UiRect::all(Val::Px(3.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(if portrait_img.is_some() {
                    Color::WHITE
                } else {
                    Color::srgb(0.16, 0.22, 0.34)
                }),
                BorderColor(Color::srgb(0.96, 0.80, 0.30)),
                portrait_img.map(ImageNode::new).unwrap_or_default(),
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.92)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(format!("Lv.{}", hs.level)),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.90, 0.36)),
                    ));
                });
            });

            // Center Vitals Column: Name + HP + Mana + XP + Skill-Up Buttons
            bar.spawn(Node {
                min_width: Val::Px(190.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|col| {
                let sp_badge = if hs.skill_points > 0 {
                    format!("  [+{} {}]", hs.skill_points, tr(lang, "hud_skill_pts"))
                } else {
                    String::new()
                };
                col.spawn((
                    Text::new(sanitize_ui_text(&format!("{h_name}{sp_badge}"))),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.92, 0.56)),
                ));

                spawn_vital_meter(
                    col,
                    &format!("HP {:.0}/{:.0}", h_ent.health, h_ent.max_health),
                    hp_pct,
                    Color::srgb(0.24, 0.92, 0.44),
                );

                if let Some(en) = energy_state {
                    let mp_pct = ((en.energy / en.max_energy.max(1.0)) * 100.0).clamp(0.0, 100.0);
                    spawn_vital_meter(
                        col,
                        &format!("MP {:.0}/{:.0}", en.energy, en.max_energy),
                        mp_pct,
                        Color::srgb(0.32, 0.68, 1.0),
                    );
                }

                spawn_vital_meter(
                    col,
                    &format!("XP {:.0}/{:.0}", h_ent.experience, next_xp),
                    xp_pct,
                    Color::srgb(0.82, 0.52, 1.0),
                );

                if hs.skill_points > 0 {
                    if let Some(spec) = hero_spec {
                        col.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: Val::Px(4.0),
                            row_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|sk_row| {
                            let skill_ids: Vec<&String> = if !spec.learnable_abilities.is_empty() {
                                spec.learnable_abilities.iter().collect()
                            } else if let Some(ud) = u_def {
                                ud.abilities.iter().map(|a| &a.id).collect()
                            } else {
                                Vec::new()
                            };
                            for ab_id in skill_ids {
                                let max_rank = u_def
                                    .and_then(|ud| ud.abilities.iter().find(|a| &a.id == ab_id))
                                    .map(|a| a.max_rank.max(3))
                                    .unwrap_or(3);
                                let rank = hs.learned_skills.get(ab_id).copied().unwrap_or(0);
                                if rank < max_rank {
                                    spawn_styled_button(
                                        sk_row,
                                        &format!("[+] {} ({}/{})", ab_id, rank, max_rank),
                                        SidebarAction::LearnHeroSkill {
                                            hero_id: h_ent.id,
                                            ability_id: ab_id.clone(),
                                        },
                                        Color::srgb(0.52, 0.38, 0.12),
                                        Val::Auto,
                                    );
                                }
                            }
                        });
                    }
                }
            });

            // Right 6-Slot Inventory Grid (`3x2`)
            bar.spawn(Node {
                width: Val::Px(156.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|inv| {
                for slot_idx in 0..6 {
                    let slot_opt = hs.inventory.get(slot_idx).and_then(|s| s.as_ref());
                    let icon = slot_icons.get(slot_idx).cloned().flatten();
                    let (short_lbl, tip_str, border_c, bg_c) = if let Some(item) = slot_opt {
                        let def_opt = loaded.items.get(&item.item_id);
                        let dname = def_opt
                            .map(|d| d.name.as_str())
                            .unwrap_or(item.item_id.as_str());
                        let chg = if item.charges > 1 {
                            format!("x{}", item.charges)
                        } else {
                            dname.chars().take(4).collect()
                        };
                        (
                            chg,
                            format!(
                                "{dname} (Slot {})\nLMB: Use Item  |  RMB: Drop on Ground",
                                slot_idx + 1
                            ),
                            Color::srgb(0.94, 0.78, 0.30),
                            if icon.is_some() {
                                Color::WHITE
                            } else {
                                Color::srgb(0.20, 0.30, 0.44)
                            },
                        )
                    } else {
                        (
                            format!("{}", slot_idx + 1),
                            format!("Empty Inventory Slot {}", slot_idx + 1),
                            Color::srgba(0.36, 0.48, 0.64, 0.55),
                            Color::srgba(0.06, 0.09, 0.14, 0.92),
                        )
                    };

                    inv.spawn((
                        Button,
                        SidebarAction::UseHeroInventorySlot {
                            hero_id: h_ent.id,
                            slot_index: slot_idx,
                        },
                        CameoTooltip(tip_str),
                        ButtonVisualStyle {
                            normal: bg_c,
                            hovered: Color::srgb(0.34, 0.48, 0.68),
                            pressed: Color::srgb(0.24, 0.36, 0.52),
                            default_border: border_c,
                        },
                        Node {
                            width: Val::Px(48.0),
                            height: Val::Px(34.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(bg_c),
                        BorderColor(border_c),
                        icon.map(ImageNode::new).unwrap_or_default(),
                    ))
                    .with_children(|s_btn| {
                        s_btn
                            .spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.84)),
                            ))
                            .with_children(|pill| {
                                pill.spawn((
                                    Text::new(sanitize_ui_text(&short_lbl)),
                                    TextFont {
                                        font_size: 9.0,
                                        ..default()
                                    },
                                    TextColor(if slot_opt.is_some() {
                                        Color::srgb(1.0, 0.96, 0.72)
                                    } else {
                                        Color::srgba(0.55, 0.66, 0.80, 0.65)
                                    }),
                                ));
                            });
                    });
                }
            });
        } else if let Some(sel_ent) = primary_selected_ent {
            // Non-hero unit/building portrait & vitals card in `ModernBottomCommandCard`
            let title = match &sel_ent.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| u.name.as_str())
                    .unwrap_or(unit_id),
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .map(|b| b.name.as_str())
                    .unwrap_or(building_id),
            };
            let hp_pct = ((sel_ent.health / sel_ent.max_health.max(1.0)) * 100.0).clamp(0.0, 100.0);
            bar.spawn((
                Node {
                    width: Val::Px(56.0),
                    height: Val::Px(56.0),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(if portrait_img.is_some() {
                    Color::WHITE
                } else {
                    Color::srgb(0.14, 0.22, 0.34)
                }),
                BorderColor(Color::srgb(0.38, 0.68, 0.96)),
                portrait_img.map(ImageNode::new).unwrap_or_default(),
            ));
            bar.spawn(Node {
                min_width: Val::Px(210.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|col| {
                let cnt = interaction.selected_ids.len();
                let header = if cnt > 1 {
                    format!("{title} (+{} selected)", cnt - 1)
                } else {
                    title.to_string()
                };
                col.spawn((
                    Text::new(sanitize_ui_text(&header)),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.90, 0.96, 1.0)),
                ));
                spawn_vital_meter(
                    col,
                    &format!("HP {:.0}/{:.0}", sel_ent.health, sel_ent.max_health),
                    hp_pct,
                    Color::srgb(0.24, 0.92, 0.44),
                );
                if let Some(en) = energy_state {
                    let mp_pct = ((en.energy / en.max_energy.max(1.0)) * 100.0).clamp(0.0, 100.0);
                    spawn_vital_meter(
                        col,
                        &format!("MP {:.0}/{:.0}", en.energy, en.max_energy),
                        mp_pct,
                        Color::srgb(0.32, 0.68, 1.0),
                    );
                }
            });
        }

        // Altar Revival Buttons for Fallen Heroes
        for fallen in &fallen_list {
            let h_name = loaded
                .units
                .get(&fallen.unit_id)
                .map(|u| u.name.as_str())
                .unwrap_or(&fallen.unit_id);
            spawn_styled_button(
                bar,
                &format!(
                    "{} {} (Lv.{})",
                    tr(lang, "hud_revive_hero"),
                    h_name,
                    fallen.hero_state.level
                ),
                SidebarAction::ReviveFallenHero(fallen.unit_id.clone()),
                Color::srgb(0.56, 0.34, 0.12),
                Val::Auto,
            );
        }
    });
}

fn spawn_vital_meter(parent: &mut ChildBuilder, label: &str, pct: f32, fill_color: Color) {
    parent
        .spawn(Node {
            width: Val::Px(186.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Node {
                    width: Val::Px(96.0),
                    height: Val::Px(6.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.11, 0.16)),
                BorderColor(Color::srgba(0.32, 0.44, 0.60, 0.7)),
            ))
            .with_children(|track| {
                track.spawn((
                    Node {
                        width: Val::Percent(pct.clamp(1.0, 100.0)),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                ));
            });
            r.spawn((
                Text::new(sanitize_ui_text(label)),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
            ));
        });
}
