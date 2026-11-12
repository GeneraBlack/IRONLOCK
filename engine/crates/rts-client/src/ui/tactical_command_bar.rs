use super::*;
use crate::input::ClientInteractionState;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::UnitRole;
use rts_simulation::world::{EntityKind, SimWorld};

/// Spawns Cameo Tooltip, `[F9]` Dev Console, and all Phase 21-28 tactical HUD overlays.
pub(super) fn spawn_phase21_24_overlays(commands: &mut Commands) {
    // 6. Floating Cameo Hover Tooltip Card
    commands
        .spawn((
            HudLayoutRole::CameoTooltipBanner,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(276.0),
                bottom: Val::Px(14.0),
                max_width: Val::Px(360.0),
                display: Display::None,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.95)),
            BorderColor(Color::srgb(0.95, 0.82, 0.34)),
            ZIndex(42),
        ))
        .with_children(|tip| {
            tip.spawn((
                HudTextRole::CameoTooltip,
                Text::new(""),
                TextFont {
                    font_size: 10.5,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.96, 1.0)),
            ));
        });

    // 7. [F9] Tactical Dev Console & Deterministic SimTickProfile Overlay
    commands
        .spawn((
            HudLayoutRole::DevConsoleOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(78.0),
                width: Val::Px(560.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.05, 0.09, 0.93)),
            BorderColor(Color::srgb(0.32, 0.78, 0.96)),
            ZIndex(45),
            PickingBehavior::IGNORE,
        ))
        .with_children(|dev| {
            dev.spawn((
                HudTextRole::DevConsoleBody,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.86, 0.96, 1.0)),
            ));
        });

    super::control_groups_bar::spawn_control_groups_bar(commands);
    super::objectives_hud::spawn_objectives_hud(commands);
    super::global_production_bar::spawn_global_production_bar(commands);
    super::hero_inventory_bar::spawn_hero_inventory_bar(commands);
    super::cutscene_director::spawn_cutscene_overlay(commands);
    super::tutorial_coach::spawn_tutorial_coach_overlay(commands);
    super::lag_shield_modal::spawn_lag_shield_modal(commands);
}

#[derive(Clone, Debug, PartialEq)]
struct AbilityBtnInfo {
    id: String,
    energy_cost: i32,
    auto_cast_capable: bool,
    auto_cast_on: bool,
    learned_rank: Option<u8>,
}

/// Rebuilds the dynamic tactical ability (with Mana cost, Hero skill rank, and golden RMB Auto-Cast
/// frame), Area-Harvest mode, squad command, `ToggleBurrow`, and `UpgradeBuildingTo` bar (`Tasks 239/240`).
pub(super) fn rebuild_tactical_ability_bar(
    commands: &mut Commands,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    pid: u8,
    last_sig: &mut LastSidebarSignature,
    ability_container_q: &Query<Entity, With<PlayerAbilityButtonsContainer>>,
) {
    let mut selected_abilities: Vec<AbilityBtnInfo> = Vec::new();
    let mut has_squad = false;
    let mut has_damaged_squad = false;
    let mut any_auto_reinforce = false;
    let mut squad_upgrade_options: Vec<String> = Vec::new();
    let mut has_subterranean = false;
    let mut any_burrowed = false;
    let mut has_harvester = false;
    let mut building_upgrade_targets: Vec<String> = Vec::new();

    for sid in &interaction.selected_ids {
        let Some(ent) = world.entities.get(sid) else {
            continue;
        };
        if ent.player_id != pid || ent.transported_by.is_some() {
            continue;
        }
        match &ent.kind {
            EntityKind::Unit { unit_id } => {
                if let Some(u) = loaded.units.get(unit_id) {
                    if u.role == UnitRole::Harvester || u.harvest_capacity > 0 {
                        has_harvester = true;
                    }
                    if selected_abilities.is_empty() && !u.abilities.is_empty() {
                        let hero_st = world.trait_stores.heroes.get(&ent.id);
                        selected_abilities = u
                            .abilities
                            .iter()
                            .enumerate()
                            .map(|(idx, a)| AbilityBtnInfo {
                                id: a.id.clone(),
                                energy_cost: a.energy_cost.round() as i32,
                                auto_cast_capable: a.allow_auto_cast,
                                auto_cast_on: ent.auto_cast_abilities.contains(&idx),
                                learned_rank: hero_st
                                    .and_then(|hs| hs.learned_skills.get(&a.id).copied()),
                            })
                            .collect();
                    }
                    if let Some(ref sq) = u.squad_trait {
                        has_squad = true;
                        if ent.squad_members_alive < sq.member_count {
                            has_damaged_squad = true;
                        }
                        if ent.squad_auto_reinforce {
                            any_auto_reinforce = true;
                        }
                        if (ent.squad_weapon_upgrades.len() as u8) < sq.weapon_upgrade_slots {
                            for upg_id in &sq.available_weapon_upgrades {
                                if !squad_upgrade_options.contains(upg_id) {
                                    squad_upgrade_options.push(upg_id.clone());
                                }
                            }
                        }
                    }
                    if u.subterranean {
                        has_subterranean = true;
                        if ent.burrowed {
                            any_burrowed = true;
                        }
                    }
                }
            }
            EntityKind::Building { building_id } => {
                if let Some(b) = loaded.buildings.get(building_id) {
                    if selected_abilities.is_empty() && !b.abilities.is_empty() {
                        selected_abilities = b
                            .abilities
                            .iter()
                            .enumerate()
                            .map(|(idx, a)| AbilityBtnInfo {
                                id: a.id.clone(),
                                energy_cost: a.energy_cost.round() as i32,
                                auto_cast_capable: a.allow_auto_cast,
                                auto_cast_on: ent.auto_cast_abilities.contains(&idx),
                                learned_rank: None,
                            })
                            .collect();
                    }
                    if ent.construction_progress >= 1.0 && ent.upgrading_to.is_none() {
                        for opt in &b.upgrade_options {
                            if !building_upgrade_targets.contains(opt) {
                                building_upgrade_targets.push(opt.clone());
                            }
                        }
                        if building_upgrade_targets.is_empty() {
                            if let Some(ref single) = b.upgrades_to {
                                building_upgrade_targets.push(single.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    let pulse_bucket = if selected_abilities.iter().any(|a| a.auto_cast_on) {
        (world.tick / 8) % 2
    } else {
        0
    };
    let ab_sig = format!(
        "ab:{selected_abilities:?}:{}:{:?}:{}:p={pulse_bucket}|hv:{}:{}|sq:{has_squad}:{has_damaged_squad}:{any_auto_reinforce}:{squad_upgrade_options:?}|sub:{has_subterranean}:{any_burrowed}|bup:{building_upgrade_targets:?}",
        interaction.selected_ability_index,
        interaction.targeted_ability_mode,
        interaction.hotkey_activate_ability,
        has_harvester,
        interaction.harvest_cursor_mode,
    );

    if ab_sig == last_sig.ability_sig {
        return;
    }
    last_sig.ability_sig = ab_sig;

    let Ok(ab_ent) = ability_container_q.get_single() else {
        return;
    };
    commands.entity(ab_ent).despawn_descendants();
    commands.entity(ab_ent).with_children(|row| {
        if selected_abilities.is_empty() {
            spawn_styled_button(
                row,
                &format!("ABILITY [{}]", interaction.hotkey_activate_ability),
                SidebarAction::TriggerAbility,
                Color::srgb(0.36, 0.28, 0.14),
                Val::Auto,
            );
        } else {
            for (idx, ab) in selected_abilities.iter().enumerate() {
                let is_active = interaction.selected_ability_index == idx
                    || interaction.targeted_ability_mode.as_deref() == Some(ab.id.as_str());
                let mp_tag = if ab.energy_cost > 0 {
                    format!(" {}MP", ab.energy_cost)
                } else {
                    String::new()
                };
                let rank_tag = ab
                    .learned_rank
                    .map(|r| format!(" R{r}"))
                    .unwrap_or_default();
                let auto_tag = if ab.auto_cast_on { " [AUTO]" } else { "" };
                let label = format!("{}{rank_tag}{mp_tag}{auto_tag} [{}]", ab.id, idx + 1);

                // Distinct golden animated/highlighted frame when Auto-Cast is enabled!
                let border_col = if ab.auto_cast_on {
                    if pulse_bucket == 0 {
                        Color::srgb(1.0, 0.88, 0.24)
                    } else {
                        Color::srgb(1.0, 0.68, 0.14)
                    }
                } else if is_active {
                    Color::srgb(0.42, 0.96, 0.62)
                } else {
                    Color::srgba(0.45, 0.68, 0.90, 0.65)
                };
                let bg_col = if ab.auto_cast_on {
                    Color::srgb(0.42, 0.32, 0.08)
                } else if is_active {
                    Color::srgb(0.24, 0.54, 0.34)
                } else {
                    Color::srgb(0.18, 0.32, 0.24)
                };
                let tip = format!(
                    "Ability: {}{}{}\nLMB: Activate / Aim (Smart-Cast if enabled)  |  RMB: Toggle Auto-Cast{}",
                    ab.id,
                    rank_tag,
                    mp_tag,
                    if ab.auto_cast_capable || ab.auto_cast_on {
                        " [Supported]"
                    } else {
                        ""
                    }
                );

                row.spawn((
                    Button,
                    SidebarAction::TriggerAbilityIndex(idx),
                    CameoTooltip(tip),
                    ButtonVisualStyle {
                        normal: bg_col,
                        hovered: Color::srgb(0.32, 0.56, 0.42),
                        pressed: Color::srgb(0.20, 0.40, 0.28),
                        default_border: border_col,
                    },
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(if ab.auto_cast_on { 2.5 } else { 1.5 })),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(bg_col),
                    BorderColor(border_col),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(sanitize_ui_text(&label)),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(if ab.auto_cast_on {
                            Color::srgb(1.0, 0.95, 0.60)
                        } else {
                            Color::WHITE
                        }),
                    ));
                });
            }
        }

        if has_harvester {
            spawn_styled_button(
                row,
                if interaction.harvest_cursor_mode {
                    "AREA HARVEST [ON]"
                } else {
                    "AREA HARVEST"
                },
                SidebarAction::ToggleHarvestCursor,
                if interaction.harvest_cursor_mode {
                    Color::srgb(0.56, 0.44, 0.12)
                } else {
                    Color::srgb(0.28, 0.24, 0.12)
                },
                Val::Auto,
            );
        }

        if has_squad {
            spawn_styled_button(
                row,
                "RETREAT [R]",
                SidebarAction::RetreatSelectedSquads,
                Color::srgb(0.48, 0.26, 0.14),
                Val::Auto,
            );
            spawn_styled_button(
                row,
                if any_auto_reinforce {
                    "AUTO-REINF: ON"
                } else {
                    "AUTO-REINF: OFF"
                },
                SidebarAction::ToggleSelectedSquadAutoReinforce,
                if any_auto_reinforce {
                    Color::srgb(0.20, 0.44, 0.34)
                } else {
                    Color::srgb(0.22, 0.24, 0.32)
                },
                Val::Auto,
            );
            if has_damaged_squad {
                spawn_styled_button(
                    row,
                    "REINFORCE NOW",
                    SidebarAction::ReinforceSelectedSquads,
                    Color::srgb(0.22, 0.48, 0.32),
                    Val::Auto,
                );
            }
            for upg_id in &squad_upgrade_options {
                let label = loaded
                    .upgrades
                    .get(upg_id)
                    .map(|u| format!("EQUIP: {}", u.name))
                    .unwrap_or_else(|| format!("EQUIP: {upg_id}"));
                spawn_styled_button(
                    row,
                    &label,
                    SidebarAction::UpgradeSelectedSquadWeapon(upg_id.clone()),
                    Color::srgb(0.26, 0.34, 0.52),
                    Val::Auto,
                );
            }
        }

        if has_subterranean {
            spawn_styled_button(
                row,
                if any_burrowed { "UNBURROW" } else { "BURROW" },
                SidebarAction::ToggleSelectedBurrow,
                if any_burrowed {
                    Color::srgb(0.38, 0.30, 0.16)
                } else {
                    Color::srgb(0.24, 0.32, 0.22)
                },
                Val::Auto,
            );
        }

        for target_b_id in &building_upgrade_targets {
            let label = loaded
                .buildings
                .get(target_b_id)
                .map(|b| format!("UPGRADE -> {} (${})", b.name, b.cost))
                .unwrap_or_else(|| format!("UPGRADE -> {target_b_id}"));
            spawn_styled_button(
                row,
                &label,
                SidebarAction::UpgradeSelectedBuildingTo(target_b_id.clone()),
                Color::srgb(0.22, 0.42, 0.48),
                Val::Auto,
            );
        }
    });
}
