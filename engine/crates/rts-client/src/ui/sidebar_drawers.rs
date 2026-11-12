use super::*;
use crate::audio::SoundSettingsResource;
use crate::input::{ClientInteractionState, UiAssetCache};
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use std::collections::HashMap;

const DRAWER_ITEMS_PER_PAGE: usize = 2;

pub(super) fn spawn_worker_builder_drawer(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    worker_buildable_ids: &[String],
    cameo_snapshot: &HashMap<String, Option<Handle<Image>>>,
) {
    let placing_worker_bid = interaction
        .worker_placing_building
        .as_ref()
        .map(|(_, bid)| bid.as_str());

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.16, 0.12)),
            BorderColor(Color::srgb(0.36, 0.72, 0.44)),
        ))
        .with_children(|box_col| {
            box_col.spawn((
                Text::new("WORKER ON-SITE CONSTRUCTION"),
                TextFont {
                    font_size: 9.5,
                    ..default()
                },
                TextColor(Color::srgb(0.75, 0.98, 0.80)),
            ));
            box_col
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::SpaceEvenly,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|grid| {
                    for b_id in worker_buildable_ids.iter().take(6) {
                        if let Some(b_def) = loaded.buildings.get(b_id) {
                            let is_active = placing_worker_bid == Some(b_id.as_str());
                            let short_name: String =
                                sanitize_ui_text(&b_def.name).chars().take(13).collect();
                            let costs = b_def.effective_resource_costs();
                            let cost_label = if costs.len() > 1 {
                                costs
                                    .iter()
                                    .map(|(k, v)| {
                                        if k == "credits" {
                                            format!("${v}")
                                        } else {
                                            format!("{v}{}", &k[..1.min(k.len())].to_uppercase())
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            } else {
                                format!("${}", b_def.cost)
                            };
                            let tip = format!(
                                "{} (Worker On-Site Build)\nCost: {cost_label} | Footprint: {}x{}\nClick cameo then LMB on terrain to dispatch selected worker(s)!",
                                b_def.name, b_def.footprint[0], b_def.footprint[1]
                            );
                            studio_layout::spawn_cameo_grid_button(
                                grid,
                                &short_name,
                                &cost_label,
                                if is_active { "SITE" } else { "BUILD" },
                                cameo_snapshot.get(b_id).cloned().flatten(),
                                SidebarAction::OrderWorkerBuild(b_id.clone()),
                                if is_active {
                                    Color::srgb(0.28, 0.56, 0.34)
                                } else {
                                    Color::srgb(0.16, 0.30, 0.20)
                                },
                                is_active,
                                None,
                                Some(tip),
                            );
                        }
                    }
                });
        });
}

pub(super) fn spawn_cameo_pagination_row(grid: &mut ChildBuilder, page: usize, total_pages: usize) {
    grid.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        padding: UiRect::top(Val::Px(2.0)),
        ..default()
    })
    .with_children(|nav_row| {
        spawn_styled_button(
            nav_row,
            "< PREV",
            SidebarAction::CycleSidebarPage(-1),
            Color::srgb(0.16, 0.26, 0.42),
            Val::Px(68.0),
        );
        nav_row.spawn((
            Text::new(format!("{}/{total_pages}", page + 1)),
            TextFont {
                font_size: 10.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.96, 1.0)),
        ));
        spawn_styled_button(
            nav_row,
            "NEXT >",
            SidebarAction::CycleSidebarPage(1),
            Color::srgb(0.16, 0.26, 0.42),
            Val::Px(68.0),
        );
    });
}

pub(super) fn spawn_support_powers_and_upgrades_drawers(
    parent: &mut ChildBuilder,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    promo_pts: u32,
    avail_support_powers: &[(rts_sdk::schema::SupportPowerDef, f32)],
    avail_upgrades: &[String],
    active_upgrade_info: Option<&(String, f32)>,
) {
    if !avail_support_powers.is_empty() {
        let sp_pages = avail_support_powers
            .len()
            .div_ceil(DRAWER_ITEMS_PER_PAGE)
            .max(1);
        let sp_page = interaction.support_power_page.min(sp_pages - 1);
        for (sp_def, rem_secs) in avail_support_powers
            .iter()
            .skip(sp_page * DRAWER_ITEMS_PER_PAGE)
            .take(DRAWER_ITEMS_PER_PAGE)
        {
            let is_targeting =
                interaction.active_support_power_id.as_deref() == Some(sp_def.id.as_str());
            let is_ready = *rem_secs <= 0.0;
            let lbl = if is_targeting {
                format!(">> SELECT TARGET: {} <<", sp_def.name)
            } else if is_ready {
                format!("[READY] {}", sp_def.name)
            } else {
                format!("[{}s] {}", rem_secs.ceil() as u32, sp_def.name)
            };
            spawn_styled_button(
                parent,
                &lbl,
                SidebarAction::TriggerSupportPower(sp_def.id.clone()),
                if is_targeting || is_ready {
                    Color::srgb(0.48, 0.34, 0.12)
                } else {
                    Color::srgb(0.20, 0.22, 0.28)
                },
                Val::Percent(100.0),
            );
        }
        if sp_pages > 1 {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        "< PWR",
                        SidebarAction::CycleSupportPowerPage(-1),
                        Color::srgb(0.18, 0.24, 0.34),
                        Val::Px(64.0),
                    );
                    row.spawn((
                        Text::new(format!("Powers {}/{sp_pages}", sp_page + 1)),
                        TextFont {
                            font_size: 9.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.90, 0.98)),
                    ));
                    spawn_styled_button(
                        row,
                        "PWR >",
                        SidebarAction::CycleSupportPowerPage(1),
                        Color::srgb(0.18, 0.24, 0.34),
                        Val::Px(64.0),
                    );
                });
        }
    }

    let eligible_upgrades: Vec<&String> = avail_upgrades
        .iter()
        .filter(|up_id| {
            loaded
                .upgrades
                .get(*up_id)
                .is_some_and(|u| !u.requires_promotion_point || promo_pts > 0)
        })
        .collect();

    if let Some((active_up_id, pct)) = active_upgrade_info {
        let up_name = loaded
            .upgrades
            .get(active_up_id)
            .map(|u| u.name.as_str())
            .unwrap_or(active_up_id.as_str());
        spawn_styled_button(
            parent,
            &format!("[RESEARCH {:.0}%] {}", pct, up_name),
            SidebarAction::ResearchUpgrade(active_up_id.clone()),
            Color::srgb(0.24, 0.38, 0.22),
            Val::Percent(100.0),
        );
    } else if !eligible_upgrades.is_empty() {
        let up_pages = eligible_upgrades
            .len()
            .div_ceil(DRAWER_ITEMS_PER_PAGE)
            .max(1);
        let up_page = interaction.upgrade_page.min(up_pages - 1);
        for &up_id in eligible_upgrades
            .iter()
            .skip(up_page * DRAWER_ITEMS_PER_PAGE)
            .take(DRAWER_ITEMS_PER_PAGE)
        {
            if let Some(up_def) = loaded.upgrades.get(up_id) {
                let cost_tag = if up_def.requires_promotion_point && up_def.cost > 0 {
                    format!("1PP+${}", up_def.cost)
                } else if up_def.requires_promotion_point {
                    "1PP".to_string()
                } else {
                    format!("${}", up_def.cost)
                };
                spawn_styled_button(
                    parent,
                    &format!("[UPG {cost_tag}] {}", up_def.name.replace('★', "*")),
                    SidebarAction::ResearchUpgrade(up_id.clone()),
                    Color::srgb(0.44, 0.32, 0.14),
                    Val::Percent(100.0),
                );
            }
        }
        if up_pages > 1 {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        "< UPG",
                        SidebarAction::CycleUpgradePage(-1),
                        Color::srgb(0.24, 0.20, 0.14),
                        Val::Px(64.0),
                    );
                    row.spawn((
                        Text::new(format!("Upgrades {}/{up_pages}", up_page + 1)),
                        TextFont {
                            font_size: 9.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.88, 0.72)),
                    ));
                    spawn_styled_button(
                        row,
                        "UPG >",
                        SidebarAction::CycleUpgradePage(1),
                        Color::srgb(0.24, 0.20, 0.14),
                        Val::Px(64.0),
                    );
                });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_player_menu_modal(
    commands: &mut Commands,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    ui_font: Option<&UiFontResource>,
    sound_res: &SoundSettingsResource,
    video_res: Option<&VideoPlaybackResource>,
    post_game_graph: Option<Handle<Image>>,
    world: &rts_simulation::world::SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    last_sig: &mut LastSidebarSignature,
    studio_mods_q: &Query<Entity, With<StudioModListContainer>>,
    pid: u8,
) {
    ui_cache.ensure_disk_catalogs(&asset_root.0);
    let map_keys: Vec<String> = loaded.maps.keys().cloned().collect();
    let lobbies_sig: String = interaction
        .online_lobbies
        .iter()
        .map(|l| format!("{}:{}:{}", l.lobby_id, l.host_addr, l.current_players))
        .collect::<Vec<_>>()
        .join(",");
    let load_step = if interaction.app_phase == crate::input::AppPhase::LoadingScreen {
        (interaction.loading_progress * 20.0).round() as i32
    } else {
        0
    };
    let snd_sig = ((sound_res.master_volume * 100.0) as u32)
        ^ (((sound_res.music_volume * 100.0) as u32) << 8)
        ^ ((sound_res.enabled as u32) << 16);
    let vid_sig = video_res
        .map(|v| format!("{}:{}:{:?}", v.is_playing, v.paused, v.active_path))
        .unwrap_or_default();
    let room_rev = interaction
        .active_lobby_room
        .as_ref()
        .map(|r| r.revision)
        .unwrap_or(0);
    let pg_tick = if interaction.active_menu_screen == crate::input::MenuScreen::PostGameScore {
        world.tick / 20
    } else {
        0
    };

    let studio_sig = format!(
        "phase={:?}:load={load_step}:screen={:?}:cfac={}:efac={}:eent={:?}:ebldg={}:ctry={}:ai={}:{:?}:{:?}:fog={}:env={:?}:srv={}:active={}:map={}:camp={:?}:{:?}:mall={}:seed={}:vid={vid_sig}:ip={}:pw={}:cname={}:am={:?}:lb={}:rrev={room_rev}:rchat={}:{}:lobbies={lobbies_sig}:spec={}:rec={:?}:xmap={}:snd={snd_sig}:ui={:.2}:edge={}:grid={}:wasd={}:ssao={}:shd={}:wm={:?}:res={:?}:vs={}:fps={}:lang={}:as={}:{}:opt={}:pg={}:{pg_tick}:rebind={:?}:hk={:?}:pause_ret={}:ws={}:{}:{:?}:{:?}:{}:{}:{:?}:mpsav={:?}:{}:mods={}:maps={}:saves={}:replays={}",
        interaction.app_phase,
        interaction.active_menu_screen,
        interaction.selected_campaign_faction,
        interaction.selected_encyclopedia_faction,
        interaction.selected_encyclopedia_entry,
        interaction.encyclopedia_show_buildings,
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
        interaction.skirmish_show_all_maps,
        interaction.procedural_map_seed,
        interaction.server_addr_input,
        interaction.lobby_password_input.len(),
        interaction.player_display_name,
        interaction.automatch_ticket_id,
        interaction.leaderboard_entries.len(),
        interaction.staging_chat_history.len(),
        interaction.staging_chat_input,
        interaction.spectator_mode_active,
        interaction.reconnect_token_cached,
        interaction.custom_map_transfer_status,
        interaction.ui_scale,
        interaction.edge_scroll_enabled,
        interaction.grid_hotkeys_enabled,
        interaction.wasd_pan_enabled,
        interaction.ssao_quality,
        interaction.shadow_quality,
        interaction.window_mode,
        interaction.resolution,
        interaction.vsync_enabled,
        interaction.fps_cap,
        interaction.language,
        interaction.autosave_interval_secs != 0,
        interaction.autosave_interval_secs,
        interaction.active_options_tab,
        interaction.post_game_tab,
        interaction.rebinding_hotkey_slot,
        interaction.hotkeys,
        interaction.settings_opened_from_pause,
        interaction.workshop_items.len(),
        interaction.workshop_search_query,
        interaction.workshop_kind_filter,
        interaction.workshop_sort,
        interaction.workshop_page,
        interaction.workshop_status_text,
        interaction.pending_missing_mod_prompt,
        interaction.selected_mp_resume_save,
        interaction.mp_resume_save_page,
        ui_cache.discovered_mods.len(),
        map_keys.join(","),
        ui_cache.save_files.len(),
        ui_cache.replay_files.len(),
    );
    let studio_sig = format!(
        "{studio_sig}:mut={:?}:rb={}:hud_ov={:?}:cb={}:sc={}:ar={}:cmut={}",
        interaction.active_mutators,
        interaction.use_hybrid_rollback,
        interaction.hud_layout_override,
        interaction.colorblind_mode,
        interaction.smart_cast_enabled,
        interaction.auto_resync_on_desync,
        ui_cache.compatible_mutators.len(),
    );

    if studio_sig != last_sig.studio_sig {
        last_sig.studio_sig = studio_sig;
        if let Ok(mod_list_ent) = studio_mods_q.get_single() {
            let discovered_mods = ui_cache.discovered_mods.clone();
            let save_files = ui_cache.save_files.clone();
            let replay_files = ui_cache.replay_files.clone();
            commands.entity(mod_list_ent).despawn_descendants();
            commands.entity(mod_list_ent).with_children(|parent| {
                super::screens::populate_active_menu_screen(
                    parent,
                    asset_server,
                    asset_root,
                    ui_cache,
                    ui_font,
                    sound_res,
                    video_res,
                    post_game_graph,
                    world,
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
