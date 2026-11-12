use super::*;
use crate::audio::SoundSettingsResource;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen, UiAssetCache, UiInputBlocker};
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::{sanitize_ui_text, UiFontResource};
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::PlayerEconomy;

use super::inspector::{build_bottom_inspector_string, update_sidebar_and_studio_lists};

/// Formats the Top Bar resource summary, rendering custom mod resources (`timber`, `alloy`, etc.)
/// dynamically alongside Credits and Power whenever `LoadedMod.resources` defines custom currencies.
pub fn format_player_resource_bar(loaded: &LoadedMod, econ: Option<&PlayerEconomy>) -> String {
    let credits = econ.map(|e| e.credits).unwrap_or(0);
    let p_prod = econ.map(|e| e.power_produced).unwrap_or(0);
    let p_cons = econ.map(|e| e.power_consumed).unwrap_or(0);
    let net_pwr = p_prod - p_cons;

    let mut parts = vec![
        format!("Credits: ${credits}"),
        format!("Power: {net_pwr:+}"),
    ];

    for (res_id, res_def) in &loaded.resources {
        if res_id == "credits" || res_id == "power" {
            continue;
        }
        let val = econ
            .and_then(|e| e.resources.get(res_id).copied())
            .unwrap_or(0);
        let label = if res_def.display_name.trim().is_empty() {
            res_id.as_str()
        } else {
            res_def.display_name.as_str()
        };
        parts.push(format!("{label}: {val}"));
    }

    parts.join("  |  ")
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_rts_hud(
    mut commands: Commands,
    (
        asset_server,
        asset_root,
        mod_res,
        sim_res,
        mut interaction,
        sound_res,
        video_res,
        post_game_graph,
        ui_font,
        mut ui_cache,
        mut ui_blocker,
        mut last_sig,
    ): (
        Option<Res<AssetServer>>,
        Res<AssetRootResource>,
        Res<ModResource>,
        Res<SimWorldResource>,
        ResMut<ClientInteractionState>,
        Res<SoundSettingsResource>,
        Option<Res<VideoPlaybackResource>>,
        Option<Res<PostGameGraphImage>>,
        Option<Res<UiFontResource>>,
        ResMut<UiAssetCache>,
        ResMut<UiInputBlocker>,
        ResMut<LastSidebarSignature>,
    ),
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut text_q: Query<(&mut Text, &HudTextRole)>,
    mut layout_q: Query<
        (&mut Node, Option<&mut BackgroundColor>, &HudLayoutRole),
        Without<StudioModalRoot>,
    >,
    mut studio_modal_q: Query<(&mut Node, &mut BackgroundColor), With<StudioModalRoot>>,
    (container_q, studio_mods_q, ability_container_q, mut hud_skin_img_q): (
        Query<Entity, With<DynamicButtonsContainer>>,
        Query<Entity, With<StudioModListContainer>>,
        Query<Entity, With<PlayerAbilityButtonsContainer>>,
        Query<(
            &mut ImageNode,
            Option<&SidebarCreditsBgNode>,
            Option<&SidebarTopBgNode>,
            Option<&SidebarOptBtnNode>,
            Option<&SidebarDiploBtnNode>,
            Option<&SidebarRadarFrameBgNode>,
        )>,
    ),
) {
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let pid = interaction.local_player_id;
    let econ = world.players.get(&pid);

    let show_ingame_hud = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;
    let show_menu_modal = (interaction.menu_open
        || interaction.app_phase == AppPhase::PostGameScore)
        && !interaction.studio_open;
    let show_menu_top_deck = show_menu_modal
        && interaction.app_phase == AppPhase::MainMenu
        && !matches!(
            interaction.active_menu_screen,
            MenuScreen::Root | MenuScreen::PauseMenu | MenuScreen::PostGameScore
        );

    // Update `ui_blocker.over_player_hud` from live `ComputedNode` HUD rectangles across all 3 presets
    if let Ok(window) = window_q.get_single() {
        ui_blocker.over_player_hud = show_ingame_hud
            && window
                .cursor_position()
                .is_some_and(|cursor| ui_blocker.pointer_over_computed_hud(cursor));
    }

    // Compute active build progress % for sidebar progress bar
    let (queue_summary, build_pct) = if let Some(ec) = econ {
        if let Some(ref ready_id) = ec.ready_building_id {
            (format!("READY TO PLACE: {ready_id}"), 100.0)
        } else if let Some((ref bid, rem, total)) = ec.building_queue {
            let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(0.0, 99.9);
            (format!("Building {bid}: {pct:.0}%"), pct)
        } else if let Some((ref up_id, rem, total)) = ec.active_upgrade {
            let pct = ((1.0 - rem / total.max(0.1)) * 100.0).clamp(0.0, 99.9);
            (format!("Researching {up_id}: {pct:.0}%"), pct)
        } else {
            ("Production Ready".to_string(), 0.0)
        }
    } else {
        ("Production Ready".to_string(), 0.0)
    };

    // Compose EVA / Chat / Victory feed banner
    let mut feed_lines: Vec<String> = Vec::new();
    if let Some(winner) = world.winner_player_id {
        if winner == pid {
            feed_lines.push("VICTORY ACCOMPLISHED — Sector Secured!".to_string());
        } else {
            feed_lines.push(format!("MISSION DEFEAT — Player {winner} Victorious."));
        }
    }
    if let Some((ref last_eva, _)) = interaction.battle_feed.last() {
        feed_lines.push(last_eva.clone());
    }
    for (chat_line, _) in interaction.chat_messages.iter().rev().take(2).rev() {
        feed_lines.push(chat_line.clone());
    }
    if interaction.chat_input_open {
        let ch = if interaction.chat_team_only {
            "TEAM"
        } else {
            "ALL"
        };
        feed_lines.push(format!("[SEND {ch}]: {}|", interaction.chat_input_buffer));
    }
    let show_battle_feed = show_ingame_hud && !feed_lines.is_empty();

    // Update all HUD layout nodes without disjoint marker conflicts
    for (mut node, bg_opt, role) in &mut layout_q {
        match role {
            HudLayoutRole::TopBarRoot | HudLayoutRole::RightSidebarRoot => {
                node.display = if show_ingame_hud {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::SidebarTelemetryStrip => {
                node.display = if show_ingame_hud {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::BottomCommandBarRoot => {
                node.display = Display::None;
            }
            HudLayoutRole::PlayerBottomStancePill => {
                node.display = if show_ingame_hud && !interaction.selected_ids.is_empty() {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::ProductionProgressFill => {
                node.width = Val::Percent(build_pct);
                if let Some(mut bg) = bg_opt {
                    *bg = BackgroundColor(if build_pct >= 99.9 {
                        Color::srgb(0.98, 0.86, 0.28)
                    } else {
                        Color::srgb(0.22, 0.92, 0.46)
                    });
                }
            }
            HudLayoutRole::BattleFeedRoot => {
                node.display = if show_battle_feed {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::StudioTopDeck => {
                node.display = if show_menu_top_deck {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::DevConsoleOverlay => {
                node.display = if show_ingame_hud && interaction.dev_console_open {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::SmokeTestEguiOverlay => {
                node.display = if interaction.studio_open {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            HudLayoutRole::CameoTooltipBanner
            | HudLayoutRole::RadarPanelRoot
            | HudLayoutRole::ClassicRadarSlotSpacer
            | HudLayoutRole::GlobalProductionStripRoot
            | HudLayoutRole::HeroInventoryBarRoot => {}
        }
    }

    // Update StudioModalRoot (Player Menu Shell) visibility
    if let Ok((mut modal_node, mut modal_bg)) = studio_modal_q.get_single_mut() {
        modal_node.display = if show_menu_modal {
            Display::Flex
        } else {
            Display::None
        };
        *modal_bg = if interaction.app_phase == AppPhase::LoadingScreen {
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.98))
        } else if interaction.app_phase == AppPhase::MainMenu
            && interaction.active_menu_screen == MenuScreen::Root
        {
            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.12))
        } else {
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.76))
        };
    }

    // Sync live radar event ring (tactical pings + friendly destruction alerts) for `[Spacebar]`
    if let Some(ping) = world.active_pings.last() {
        if interaction.last_synced_radar_event != Some(ping.position) {
            interaction.last_synced_radar_event = Some(ping.position);
            interaction.push_radar_event(ping.position);
        }
    } else if let Some(dest) = world.destruction_events.iter().find(|d| d.player_id == pid) {
        if interaction.last_synced_radar_event != Some(dest.position) {
            interaction.last_synced_radar_event = Some(dest.position);
            interaction.push_radar_event(dest.position);
        }
    }

    // Update all HUD text nodes via `HudTextRole`
    let idle_cnt = crate::input::collect_idle_economy_units(world, loaded, pid).len();
    let top_res_summary = {
        let secs = world.tick / 20;
        let pause_tag = if interaction.game_paused {
            "  [PAUSED]"
        } else if interaction.replay_active {
            "  [REPLAY]"
        } else {
            ""
        };
        format!(
            "{}  |  Idle: {idle_cnt}  |  {:02}:{:02} ({:.1}x){pause_tag}",
            format_player_resource_bar(loaded, econ),
            secs / 60,
            secs % 60,
            interaction.sim_speed_multiplier
        )
    };
    let credits_readout = econ
        .map(|e| format!("$ {:05}", e.credits))
        .unwrap_or_else(|| "$ 00000".to_string());
    let inspector_str = if !interaction.selected_ids.is_empty() {
        build_bottom_inspector_string(world, loaded, &interaction)
    } else {
        String::new()
    };

    for (mut txt, role) in &mut text_q {
        match role {
            HudTextRole::TopBarStatus => {
                **txt = sanitize_ui_text(&top_res_summary);
            }
            HudTextRole::SidebarQueue => {
                **txt = sanitize_ui_text(&queue_summary);
            }
            HudTextRole::SidebarCredits => {
                **txt = credits_readout.clone();
            }
            HudTextRole::PlayerBottomStance | HudTextRole::BottomInspector => {
                if !inspector_str.is_empty() {
                    **txt = sanitize_ui_text(&inspector_str);
                }
            }
            HudTextRole::BattleFeed => {
                **txt = sanitize_ui_text(&feed_lines.join("\n"));
            }
            HudTextRole::StudioStatus => {
                **txt = sanitize_ui_text(&interaction.studio_status_message);
            }
            HudTextRole::StudioHeaderTitle => {
                **txt = sanitize_ui_text(&format!(
                    "{} — {}",
                    loaded.manifest.name.to_uppercase(),
                    interaction.active_menu_screen.label()
                ));
            }
            HudTextRole::DevConsoleBody => {
                if show_ingame_hud && interaction.dev_console_open {
                    **txt = sanitize_ui_text(&format_dev_console_profiler(world, &interaction));
                }
            }
            HudTextRole::CameoTooltip => {}
        }
    }

    // Apply faction-specific HUD skin textures via `UiAssetCache`
    let active_fac_id = econ.map(|e| e.faction.as_str()).unwrap_or("");
    if let Some(hud_skin) = crate::input::resolve_active_faction_hud_skin(loaded, active_fac_id) {
        for (mut img_node, is_cred, is_top, is_opt, is_diplo, is_radar) in &mut hud_skin_img_q {
            let rel_opt = if is_cred.is_some() {
                hud_skin.credits_panel.as_deref()
            } else if is_top.is_some() {
                hud_skin.top_panel.as_deref()
            } else if is_opt.is_some() {
                hud_skin.options_button.as_deref()
            } else if is_diplo.is_some() {
                hud_skin.diplomacy_button.as_deref()
            } else if is_radar.is_some() {
                hud_skin.radar_frame.as_deref()
            } else {
                None
            };
            if let Some(rel) = rel_opt {
                if let Some(h) = ui_cache.load_qualified_mod_image(
                    loaded,
                    asset_server.as_deref(),
                    &asset_root.0,
                    rel,
                ) {
                    if img_node.image != h {
                        img_node.image = h;
                    }
                }
            }
        }
    }

    // Dynamic Multi-Ability, Squad & Building Upgrade Bar on Bottom Selection Pill
    if show_ingame_hud {
        super::tactical_command_bar::rebuild_tactical_ability_bar(
            &mut commands,
            world,
            loaded,
            &interaction,
            pid,
            &mut last_sig,
            &ability_container_q,
        );
    }

    update_sidebar_and_studio_lists(
        &mut commands,
        asset_server.as_deref(),
        &asset_root,
        &mut ui_cache,
        ui_font.as_deref(),
        &sound_res,
        video_res.as_deref(),
        post_game_graph.as_ref().map(|g| g.0.clone()),
        world,
        loaded,
        &interaction,
        pid,
        &mut last_sig,
        &container_q,
        &studio_mods_q,
    );
}

pub fn update_cameo_tooltip_ui(
    interaction: Res<ClientInteractionState>,
    mut banner_q: Query<(&mut Node, &HudLayoutRole)>,
    mut text_q: Query<(&mut Text, &HudTextRole)>,
) {
    let show = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open
        && interaction.hovered_cameo_tooltip.is_some();

    for (mut node, role) in &mut banner_q {
        if *role == HudLayoutRole::CameoTooltipBanner {
            node.display = if show { Display::Flex } else { Display::None };
        }
    }
    if let Some(ref tip) = interaction.hovered_cameo_tooltip {
        for (mut txt, role) in &mut text_q {
            if *role == HudTextRole::CameoTooltip {
                **txt = sanitize_ui_text(tip);
            }
        }
    }
}

fn format_dev_console_profiler(
    world: &rts_simulation::world::SimWorld,
    interaction: &ClientInteractionState,
) -> String {
    let p = &world.last_tick_profile;
    let flags = &interaction.debug_gizmo_flags;
    let mut lines = vec![
        "[F9] TACTICAL DEV CONSOLE & DETERMINISTIC SIM PROFILER".to_string(),
        format!(
            "Tick: {}  |  State Hash: 0x{:08x}  |  Entities: {}  |  Projectiles: {}  |  Path Cache Hits: {}",
            world.tick,
            world.state_hash() as u32,
            p.entity_count,
            p.projectile_count,
            p.path_cache_hits
        ),
        format!(
            "Tick Budget: Total {} us (Cmd {} | Move {} | Combat {} | Proj {} | Trig/FoW {} us)",
            p.total_us,
            p.commands_us,
            p.movement_us,
            p.combat_us,
            p.projectiles_us,
            p.triggers_fow_us
        ),
        format!(
            "Gizmos: Paths={} | Ranges={} | Vision={} | Zones={} | Blocked={}",
            if flags.show_paths { "ON" } else { "OFF" },
            if flags.show_weapon_ranges { "ON" } else { "OFF" },
            if flags.show_vision_and_stealth { "ON" } else { "OFF" },
            if flags.show_trigger_zones { "ON" } else { "OFF" },
            if flags.show_blocked_tiles { "ON" } else { "OFF" },
        ),
    ];
    for entry in interaction.dev_console_history.iter().rev().take(3).rev() {
        lines.push(format!("  {entry}"));
    }
    lines.push(format!("> {}_", interaction.dev_console_input));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_resource_top_bar_renders_custom_resources_when_defined() {
        let showcase_dir = LoadedMod::resolve_mod_dir("sdk-showcase-workers-squads");
        if showcase_dir.exists() {
            let loaded = LoadedMod::load_from_dir(showcase_dir).expect("load showcase mod");
            let world = rts_simulation::world::SimWorld::from_mod(&loaded);
            let bar = format_player_resource_bar(&loaded, world.players.get(&1));
            assert!(bar.contains("Credits:"), "expected Credits in '{bar}'");
            assert!(
                bar.to_ascii_lowercase().contains("timber")
                    || bar.to_ascii_lowercase().contains("alloy"),
                "expected custom resources in '{bar}'"
            );
        }
    }
}
