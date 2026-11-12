use super::{spawn_artwork_banner, spawn_body_text, spawn_screen_card};
use crate::input::{
    mod_skirmish_option_count, resolve_mod_skirmish_descriptor, ClientInteractionState,
    UiAssetCache,
};
use crate::ui::studio_layout::{spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_text_input, UiScrollContainer, UiScrollContent, UiTextFieldTarget};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::ai::AiDifficulty;

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
    let opt_count = mod_skirmish_option_count(loaded);
    let (country_name, fac_id, specialty, art_path) =
        resolve_mod_skirmish_descriptor(loaded, interaction.skirmish_country_index);

    spawn_screen_card(
        parent,
        32.0,
        "1. FACTION & DOCTRINE SELECTOR",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            spawn_artwork_banner(
                col,
                ui_cache,
                asset_server,
                asset_root,
                art_path.as_deref(),
                135.0,
            );
            spawn_body_text(
                col,
                format!("Active: {country_name} ({fac_id})\nSpecialty: {specialty}"),
                Color::srgb(0.84, 0.92, 0.98),
            );

            let max_scroll = (opt_count as f32 * 34.0 - 220.0).max(0.0);
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
                    for idx in 0..opt_count {
                        let (cname, cfid, _, _) = resolve_mod_skirmish_descriptor(loaded, idx);
                        let selected = idx == interaction.skirmish_country_index % opt_count;
                        spawn_styled_button(
                            list,
                            &format!(
                                "{} {} ({})",
                                if selected { "[*]" } else { " > " },
                                cname,
                                cfid
                            ),
                            SidebarAction::SelectSkirmishCountryIndex(idx),
                            if selected {
                                Color::srgb(0.22, 0.48, 0.30)
                            } else {
                                Color::srgb(0.16, 0.24, 0.36)
                            },
                            Val::Percent(100.0),
                        );
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
                    "AI Opponent: ACTIVE (Strategic Commander)"
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

            // Compatible Mutator / Overlay Packages (`Task 246-LobbyUI`)
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
                "Includes Influence-Map Scouting, Counter-Unit Production & Multi-Resource Economy.",
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
