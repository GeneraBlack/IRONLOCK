use super::{spawn_artwork_banner, spawn_body_text, spawn_screen_card};
use crate::input::{
    mod_skirmish_option_count, resolve_mod_skirmish_descriptor,
    resolve_mod_skirmish_sub_faction_id, ClientInteractionState, UiAssetCache,
};
use crate::ui::studio_layout::{spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_text_input, UiScrollContainer, UiScrollContent, UiTextFieldTarget};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{FactionDef, SubFactionDef};
use rts_simulation::ai::AiDifficulty;

pub const RANDOM_FLAG_STRIPES: [[f32; 3]; 3] =
    [[0.18, 0.48, 0.92], [0.92, 0.22, 0.20], [0.72, 0.24, 0.90]];

/// Resolves `(parent_faction, sub_faction)` for `sub_id` from `loaded.factions`.
pub fn resolve_country_def_by_id<'a>(
    loaded: &'a LoadedMod,
    sub_id: &str,
) -> Option<(&'a FactionDef, &'a SubFactionDef)> {
    let norm = if sub_id.eq_ignore_ascii_case("america") {
        "usa"
    } else {
        sub_id.trim()
    };
    for fac in loaded.factions.values() {
        if let Some(sf) = fac
            .sub_factions
            .iter()
            .find(|sf| sf.id.eq_ignore_ascii_case(norm))
        {
            return Some((fac, sf));
        }
    }
    None
}

/// Resolves friendly signature unit/building display name for a `SubFactionDef`.
pub fn resolve_signature_unit_label(loaded: &LoadedMod, sf: &SubFactionDef) -> String {
    if let Some(ref sig_id) = sf.signature_unit {
        if let Some(u) = loaded.units.get(sig_id) {
            return format!("{} (${})", u.name, u.cost);
        }
        if let Some(b) = loaded.buildings.get(sig_id) {
            return format!("{} (${})", b.name, b.cost);
        }
    }
    sf.specialty.clone()
}

/// Spawns a crisp 3-stripe miniature country flag badge (`24x15 px`).
pub fn spawn_country_flag_badge(parent: &mut ChildBuilder, stripes: [[f32; 3]; 3]) {
    parent
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(15.0),
                min_width: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                border: UiRect::all(Val::Px(1.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.06, 0.08)),
            BorderColor(Color::srgba(0.86, 0.90, 0.96, 0.85)),
        ))
        .with_children(|flag| {
            for [r, g, b] in stripes {
                flag.spawn((
                    Node {
                        width: Val::Percent(33.34),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(
                        r.clamp(0.0, 1.0),
                        g.clamp(0.0, 1.0),
                        b.clamp(0.0, 1.0),
                    )),
                ));
            }
        });
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_skirmish_screen(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    _pid: u8,
) {
    let opt_count = mod_skirmish_option_count(loaded).max(1);
    let is_random = interaction.skirmish_country_index == usize::MAX;
    let (country_name, fac_id, specialty, art_path, active_stripes, bonus_line, sig_label) =
        if is_random {
            (
                "Random Country (All 10 Nations)".to_string(),
                "random".to_string(),
                "Deterministic Seed Roll across Allies, Soviets & Psi-Corps".to_string(),
                loaded
                    .factions
                    .values()
                    .next()
                    .and_then(|f| f.loading_screen.as_deref())
                    .map(|rel| crate::input::qualify_mod_asset_rel_path(loaded, rel)),
                RANDOM_FLAG_STRIPES,
                "Rolls authentic country + passive bonuses at battle start".to_string(),
                "Country-Exclusive Arsenal".to_string(),
            )
        } else {
            let (cname, fid, spec, art) =
                resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);
            let sub_opt =
                resolve_mod_skirmish_sub_faction_id(loaded, interaction.skirmish_country_index)
                    .and_then(|sid| resolve_country_def_by_id(loaded, &sid));
            let (stripes, bonus, sig) = if let Some((f_def, sf_def)) = sub_opt {
                (
                    sf_def.resolved_flag_colors(f_def.color),
                    sf_def.bonus_summary(),
                    resolve_signature_unit_label(loaded, sf_def),
                )
            } else {
                let fallback = loaded
                    .factions
                    .get(&fid)
                    .map(|f| f.color)
                    .unwrap_or([0.4, 0.5, 0.7]);
                (
                    [fallback, [0.92, 0.94, 0.96], fallback],
                    spec.clone(),
                    spec.clone(),
                )
            };
            (cname, fid, spec, art, stripes, bonus, sig)
        };

    spawn_screen_card(
        parent,
        32.0,
        "1. COUNTRY & DOCTRINE SELECTOR",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            spawn_artwork_banner(
                col,
                ui_cache,
                asset_server,
                asset_root,
                art_path.as_deref(),
                118.0,
            );
            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|hdr| {
                spawn_country_flag_badge(hdr, active_stripes);
                spawn_body_text(
                    hdr,
                    format!("{country_name} [{fac_id}]"),
                    Color::srgb(0.96, 0.92, 0.68),
                );
            });
            spawn_body_text(
                col,
                format!(
                    "Signature: {sig_label}\nPassive Bonus: {bonus_line}\nDoctrine: {specialty}"
                ),
                Color::srgb(0.84, 0.92, 0.98),
            );
            let (swatch_name, [sr, sg, sb]) = crate::rendering::resolve_ui_color_swatch(
                interaction.skirmish_color_index,
                (!is_random).then_some(fac_id.as_str()),
                Some(loaded),
            );
            spawn_styled_button(
                col,
                &format!("Team Color: {swatch_name} (Click to Cycle)"),
                SidebarAction::CycleSkirmishColor,
                Color::srgb(
                    (sr * 0.62).clamp(0.10, 0.85),
                    (sg * 0.62).clamp(0.10, 0.85),
                    (sb * 0.62).clamp(0.10, 0.85),
                ),
                Val::Percent(100.0),
            );

            let max_scroll = ((opt_count + 1) as f32 * 36.0 - 205.0).max(0.0);
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
                    // "Random" country selector row
                    list.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_country_flag_badge(row, RANDOM_FLAG_STRIPES);
                        spawn_styled_button(
                            row,
                            &format!(
                                "{} Random Country (All 10 Nations)",
                                if is_random { "[*]" } else { "[?]" }
                            ),
                            SidebarAction::SelectSkirmishCountryIndex(usize::MAX),
                            if is_random {
                                Color::srgb(0.26, 0.48, 0.34)
                            } else {
                                Color::srgb(0.20, 0.24, 0.38)
                            },
                            Val::Percent(90.0),
                        );
                    });

                    for idx in 0..opt_count {
                        let (cname, cfid, _, _) = resolve_mod_skirmish_descriptor(loaded, idx);
                        let sub_pair = resolve_mod_skirmish_sub_faction_id(loaded, idx)
                            .and_then(|sid| resolve_country_def_by_id(loaded, &sid));
                        let (stripes, short_bonus) = if let Some((f_def, sf_def)) = sub_pair {
                            (
                                sf_def.resolved_flag_colors(f_def.color),
                                sf_def.bonus_summary(),
                            )
                        } else {
                            (RANDOM_FLAG_STRIPES, cfid.clone())
                        };
                        let selected =
                            !is_random && idx == interaction.skirmish_country_index % opt_count;
                        list.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(5.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_country_flag_badge(row, stripes);
                            spawn_styled_button(
                                row,
                                &format!(
                                    "{} {} — {}",
                                    if selected { "[*]" } else { " > " },
                                    cname,
                                    short_bonus
                                ),
                                SidebarAction::SelectSkirmishCountryIndex(idx),
                                if selected {
                                    Color::srgb(0.22, 0.48, 0.30)
                                } else {
                                    Color::srgb(0.16, 0.24, 0.36)
                                },
                                Val::Percent(90.0),
                            );
                        });
                    }
                });
            });
        },
    );

    spawn_screen_card(
        parent,
        34.0,
        "2. BATTLEFIELD MAP & PROCEDURAL SEED",
        Color::srgb(0.65, 0.88, 1.0),
        |col| {
            let campaign_map_ids: std::collections::HashSet<&str> = loaded
                .campaigns
                .values()
                .flat_map(|c| c.missions.iter().map(|m| m.map_id.as_str()))
                .collect();
            let mut map_list: Vec<(&String, &rts_sdk::schema::MapDef)> = loaded
                .maps
                .iter()
                .filter(|(id, _)| {
                    interaction.skirmish_show_all_maps
                        || id.starts_with("mp_")
                        || (!campaign_map_ids.contains(id.as_str()) && !id.contains("mission"))
                })
                .collect();
            if map_list.is_empty() {
                map_list = loaded.maps.iter().collect();
            }
            map_list.sort_by_key(|(id, _)| id.as_str());

            spawn_styled_button(
                col,
                if interaction.skirmish_show_all_maps {
                    "Filter: Showing ALL Maps (Click for Skirmish Only)"
                } else {
                    "Filter: Skirmish Maps Only (Click for All Maps)"
                },
                SidebarAction::ToggleSkirmishMapFilter,
                Color::srgb(0.20, 0.28, 0.42),
                Val::Percent(100.0),
            );

            let max_scroll = (map_list.len() as f32 * 34.0 - 210.0).max(0.0);
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
                    for (mid, mdef) in map_list {
                        let active = interaction.active_map_id == *mid
                            || (interaction.active_map_id.is_empty()
                                && mdef.name == loaded.entry_map.name);
                        spawn_styled_button(
                            list,
                            &format!(
                                "{} {} ({}x{})",
                                if active { "[*]" } else { " > " },
                                mdef.name,
                                mdef.width,
                                mdef.height
                            ),
                            SidebarAction::SwitchActiveMap(mid.clone()),
                            if active {
                                Color::srgb(0.22, 0.46, 0.34)
                            } else {
                                Color::srgb(0.16, 0.22, 0.32)
                            },
                            Val::Percent(100.0),
                        );
                    }
                });
            });

            spawn_text_input(
                col,
                ui_font,
                "Procedural Tournament Map Seed (Click to Edit):",
                UiTextFieldTarget::ProceduralSeed,
                &interaction.procedural_map_seed.to_string(),
                false,
                false,
            );
            spawn_styled_button(
                col,
                "GENERATE TOURNAMENT SKIRMISH MAP",
                SidebarAction::GenerateProceduralSkirmishMap,
                Color::srgb(0.26, 0.36, 0.20),
                Val::Percent(100.0),
            );
        },
    );

    spawn_screen_card(
        parent,
        34.0,
        "3. AI COMMANDER & MATCH RULES",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            spawn_styled_button(
                col,
                if interaction.ai_enabled {
                    "AI Opponent: ACTIVE (Auto-Rolls Authentic Country)"
                } else {
                    "AI Opponent: SANDBOX IDLE"
                },
                SidebarAction::ToggleSkirmishAi,
                if interaction.ai_enabled {
                    Color::srgb(0.18, 0.40, 0.28)
                } else {
                    Color::srgb(0.38, 0.22, 0.20)
                },
                Val::Percent(100.0),
            );

            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for (diff, lbl) in [
                    (AiDifficulty::Easy, "EASY"),
                    (AiDifficulty::Medium, "MED"),
                    (AiDifficulty::Hard, "HARD"),
                    (AiDifficulty::Brutal, "BRUTAL"),
                ] {
                    spawn_styled_button(
                        row,
                        lbl,
                        SidebarAction::SetAiDifficulty(diff),
                        if interaction.ai_difficulty == diff {
                            Color::srgb(0.26, 0.48, 0.34)
                        } else {
                            Color::srgb(0.18, 0.24, 0.34)
                        },
                        Val::Percent(25.0),
                    );
                }
            });

            spawn_styled_button(
                col,
                &format!("AI Doctrine: {:?}", interaction.ai_personality),
                SidebarAction::CycleAiPersonality,
                Color::srgb(0.22, 0.30, 0.44),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                if interaction.fog_of_war_enabled {
                    "Fog of War: ENABLED"
                } else {
                    "Fog of War: DISABLED (Full Vision)"
                },
                SidebarAction::ToggleFogOfWar,
                Color::srgb(0.20, 0.32, 0.38),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                col,
                &format!("Atmosphere: {}", interaction.environment_preset.label()),
                SidebarAction::CycleEnvironmentPreset,
                Color::srgb(0.22, 0.28, 0.42),
                Val::Percent(100.0),
            );

            let (mutators, cross_mod) =
                super::mp_staging_rules::collect_compatible_and_cross_mod_mutators(
                    ui_cache, loaded,
                );
            spawn_body_text(
                col,
                format!(
                    "Mutator Overlays ({} active):",
                    interaction.active_mutators.len()
                ),
                Color::srgb(0.98, 0.86, 0.44),
            );
            if !mutators.is_empty() {
                for mut_entry in mutators.iter().take(4) {
                    let enabled = interaction
                        .active_mutators
                        .iter()
                        .any(|m| m == &mut_entry.id);
                    let mark = if enabled { "[X]" } else { "[ ]" };
                    spawn_styled_button(
                        col,
                        &format!("{mark} {}", mut_entry.name),
                        SidebarAction::ToggleMutatorOverlay(mut_entry.id.clone()),
                        if enabled {
                            Color::srgb(0.38, 0.32, 0.14)
                        } else {
                            Color::srgb(0.16, 0.20, 0.28)
                        },
                        Val::Percent(100.0),
                    );
                }
            } else if let Some((cross_name, req_mod)) = cross_mod.first() {
                spawn_body_text(
                    col,
                    format!("[—] {cross_name} ({req_mod})"),
                    Color::srgb(0.62, 0.72, 0.84),
                );
            }

            spawn_body_text(
                col,
                "All 10 RA2/YR Countries active: passive bonuses & AI country rolls enabled.",
                Color::srgb(0.75, 0.86, 0.94),
            );

            spawn_styled_button(
                col,
                "DEPLOY SKIRMISH BATTLE",
                SidebarAction::EnterLoadingScreen(format!(
                    "SKIRMISH — {country_name} ON {}",
                    loaded.entry_map.name.to_uppercase()
                )),
                Color::srgb(0.18, 0.52, 0.28),
                Val::Percent(100.0),
            );
        },
    );
}
