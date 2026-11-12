use super::{spawn_artwork_banner, spawn_body_text, spawn_screen_card};
use crate::input::{
    qualify_mod_asset_rel_path, resolve_mod_campaign_loading_screen, ClientInteractionState,
    UiAssetCache,
};
use crate::ui::studio_layout::{spawn_icon_styled_button, spawn_styled_button};
use crate::ui::widgets::{UiScrollContainer, UiScrollContent};
use crate::ui::SidebarAction;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::ai::AiDifficulty;

pub fn spawn_campaign_screen(
    parent: &mut ChildBuilder,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) {
    let active_fac_id = if !interaction.selected_campaign_faction.is_empty()
        && loaded
            .factions
            .contains_key(&interaction.selected_campaign_faction)
    {
        interaction.selected_campaign_faction.clone()
    } else {
        loaded
            .factions
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "allies".to_string())
    };

    let selected_camp_id = interaction
        .active_campaign_id
        .clone()
        .filter(|cid| loaded.campaigns.contains_key(cid))
        .or_else(|| {
            loaded
                .campaigns
                .iter()
                .find(|(_, c)| c.faction.eq_ignore_ascii_case(&active_fac_id))
                .map(|(id, _)| id.clone())
        })
        .or_else(|| loaded.campaigns.keys().next().cloned())
        .unwrap_or_default();

    let multi_store = rts_sdk::schema::MultiCampaignSaveStore::load_or_default(
        std::path::Path::new("saves/campaign_progress.json"),
    );
    let profile = multi_store
        .get_profile(&loaded.manifest.id, &selected_camp_id)
        .cloned()
        .or_else(|| {
            rts_sdk::schema::CampaignSaveProfile::load_from_file(std::path::Path::new(
                "saves/campaign_profile.json",
            ))
            .ok()
        })
        .unwrap_or_default();

    let camp_cards: Vec<(String, String, Option<Handle<Image>>, Color)> = loaded
        .campaigns
        .iter()
        .map(|(camp_id, camp_def)| {
            let is_sel = camp_id == &selected_camp_id;
            let [r, g, b] = loaded
                .factions
                .get(&camp_def.faction)
                .map(|f| f.color)
                .unwrap_or([0.35, 0.55, 0.85]);
            let col = if is_sel {
                Color::srgb(
                    (r * 0.65 + 0.14).min(0.85),
                    (g * 0.65 + 0.14).min(0.85),
                    (b * 0.65 + 0.14).min(0.85),
                )
            } else {
                Color::srgb(
                    (r * 0.32 + 0.08).min(0.55),
                    (g * 0.32 + 0.08).min(0.55),
                    (b * 0.32 + 0.08).min(0.55),
                )
            };
            let cameo = loaded
                .units
                .iter()
                .find(|(_, u)| u.faction.eq_ignore_ascii_case(&camp_def.faction))
                .and_then(|(uid, _)| ui_cache.load_entity_cameo(loaded, asset_server, uid));
            (camp_id.clone(), camp_def.display_name.clone(), cameo, col)
        })
        .collect();

    spawn_screen_card(
        parent,
        28.0,
        "1. THEATRE OF OPERATIONS",
        Color::srgb(0.98, 0.84, 0.42),
        |col| {
            for (cid, title, cameo, col_bg) in camp_cards {
                spawn_icon_styled_button(
                    col,
                    &title,
                    cameo,
                    SidebarAction::SelectCampaignById(cid),
                    col_bg,
                    Val::Percent(100.0),
                );
            }
            spawn_body_text(
                col,
                format!(
                    "Completed: {}  |  Reserve: +${}  |  Veterans: {}",
                    profile.completed_missions.len(),
                    profile.carry_over_credits,
                    profile.carry_over_units.len()
                ),
                Color::srgb(0.62, 0.94, 0.76),
            );
            spawn_body_text(
                col,
                format!("Difficulty: {}", interaction.ai_difficulty.label()),
                Color::srgb(0.95, 0.88, 0.55),
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
        },
    );

    spawn_screen_card(
        parent,
        36.0,
        "2. STORY MISSIONS (MOUSE-WHEEL SCROLL)",
        Color::srgb(0.65, 0.86, 1.0),
        |col| {
            if let Some(camp_def) = loaded.campaigns.get(&selected_camp_id) {
                let max_scroll = (camp_def.missions.len() as f32 * 38.0 - 380.0).max(0.0);
                col.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(0.0),
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        overflow: Overflow::clip_y(),
                        padding: UiRect::all(Val::Px(4.0)),
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
                            row_gap: Val::Px(5.0),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        for (idx, miss) in camp_def.missions.iter().enumerate() {
                            let is_done = profile.completed_missions.contains(&miss.id);
                            let prev_done = idx == 0
                                || camp_def
                                    .missions
                                    .get(idx - 1)
                                    .is_some_and(|p| profile.completed_missions.contains(&p.id));
                            let locked = !prev_done && !is_done;
                            let active =
                                interaction.active_mission_id.as_deref() == Some(miss.id.as_str());
                            let badge = if locked {
                                "[LOCK]"
                            } else if is_done {
                                "[OK]"
                            } else if active {
                                "[SELECTED]"
                            } else {
                                "[DEPLOY]"
                            };
                            spawn_styled_button(
                                list,
                                &format!("{badge} {:02}. {}", idx + 1, miss.display_name),
                                SidebarAction::StartCampaignMission {
                                    campaign_id: selected_camp_id.clone(),
                                    mission_id: miss.id.clone(),
                                },
                                if locked {
                                    Color::srgb(0.20, 0.16, 0.18)
                                } else if active {
                                    Color::srgb(0.22, 0.48, 0.28)
                                } else {
                                    Color::srgb(0.18, 0.28, 0.40)
                                },
                                Val::Percent(100.0),
                            );
                        }
                    });
                });
            }
        },
    );

    spawn_screen_card(
        parent,
        36.0,
        "3. BRIEFING & IN-ENGINE FMV PREVIEW",
        Color::srgb(0.55, 0.95, 0.72),
        |col| {
            let active_camp = loaded.campaigns.get(&selected_camp_id);
            let active_miss = active_camp.and_then(|c| {
                interaction
                    .active_mission_id
                    .as_ref()
                    .and_then(|mid| c.missions.iter().find(|m| &m.id == mid))
                    .or_else(|| c.missions.first())
            });
            let art = active_miss
                .and_then(|m| {
                    m.loading_screen
                        .as_ref()
                        .map(|p| qualify_mod_asset_rel_path(loaded, p))
                })
                .or_else(|| resolve_mod_campaign_loading_screen(loaded, &active_fac_id, 0));
            spawn_artwork_banner(
                col,
                ui_cache,
                asset_server,
                asset_root,
                art.as_deref(),
                150.0,
            );

            if let Some(miss) = active_miss {
                spawn_body_text(
                    col,
                    format!("OPERATION: {}  (Map: {})", miss.display_name, miss.map_id),
                    Color::srgb(0.98, 0.88, 0.45),
                );
                spawn_body_text(col, &miss.briefing, Color::srgb(0.86, 0.91, 0.96));
                if let Some(vid) = miss.intro_video.as_ref().or(miss.radar_video.as_ref()) {
                    spawn_styled_button(
                        col,
                        "PLAY IN-ENGINE BRIEFING FMV",
                        SidebarAction::PlayModVideo(vid.clone()),
                        Color::srgb(0.38, 0.24, 0.16),
                        Val::Percent(100.0),
                    );
                }
                spawn_styled_button(
                    col,
                    "LAUNCH MISSION",
                    SidebarAction::StartCampaignMission {
                        campaign_id: selected_camp_id.clone(),
                        mission_id: miss.id.clone(),
                    },
                    Color::srgb(0.18, 0.48, 0.28),
                    Val::Percent(100.0),
                );
            }
        },
    );
}
