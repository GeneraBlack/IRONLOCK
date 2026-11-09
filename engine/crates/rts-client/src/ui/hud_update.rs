use super::*;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen};
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::world::EntityKind;

use super::inspector::{build_bottom_inspector_string, update_sidebar_and_studio_lists};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_rts_hud(
    mut commands: Commands,
    (asset_server, asset_root, mod_res, sim_res, interaction, mut last_sig): (
        Option<Res<AssetServer>>,
        Res<AssetRootResource>,
        Res<ModResource>,
        Res<SimWorldResource>,
        Res<ClientInteractionState>,
        ResMut<LastSidebarSignature>,
    ),
    mut top_text_q: Query<
        &mut Text,
        (
            With<TopBarStatusText>,
            Without<SidebarQueueText>,
            Without<BottomInspectorText>,
            Without<StudioStatusText>,
            Without<BattleFeedText>,
            Without<StudioHeaderTitleText>,
            Without<StudioTab6ButtonText>,
            Without<SidebarCreditsText>,
            Without<PlayerBottomStanceText>,
        ),
    >,
    mut queue_text_q: Query<
        &mut Text,
        (
            With<SidebarQueueText>,
            Without<TopBarStatusText>,
            Without<BottomInspectorText>,
            Without<StudioStatusText>,
            Without<BattleFeedText>,
            Without<StudioHeaderTitleText>,
            Without<StudioTab6ButtonText>,
            Without<SidebarCreditsText>,
            Without<PlayerBottomStanceText>,
        ),
    >,
    mut bottom_text_q: Query<
        &mut Text,
        (
            With<BottomInspectorText>,
            Without<TopBarStatusText>,
            Without<SidebarQueueText>,
            Without<StudioStatusText>,
            Without<BattleFeedText>,
            Without<StudioHeaderTitleText>,
            Without<StudioTab6ButtonText>,
            Without<SidebarCreditsText>,
            Without<PlayerBottomStanceText>,
        ),
    >,
    mut studio_status_q: Query<
        &mut Text,
        (
            With<StudioStatusText>,
            Without<TopBarStatusText>,
            Without<SidebarQueueText>,
            Without<BottomInspectorText>,
            Without<BattleFeedText>,
            Without<StudioHeaderTitleText>,
            Without<StudioTab6ButtonText>,
            Without<SidebarCreditsText>,
            Without<PlayerBottomStanceText>,
        ),
    >,
    mut feed_text_q: Query<
        &mut Text,
        (
            With<BattleFeedText>,
            Without<TopBarStatusText>,
            Without<SidebarQueueText>,
            Without<BottomInspectorText>,
            Without<StudioStatusText>,
            Without<StudioHeaderTitleText>,
            Without<StudioTab6ButtonText>,
            Without<SidebarCreditsText>,
            Without<PlayerBottomStanceText>,
        ),
    >,
    mut studio_hdr_text_q: Query<
        (
            &mut Text,
            Option<&StudioHeaderTitleText>,
            Option<&StudioTab6ButtonText>,
            Option<&SidebarCreditsText>,
            Option<&PlayerBottomStanceText>,
        ),
        (
            Or<(
                With<StudioHeaderTitleText>,
                With<StudioTab6ButtonText>,
                With<SidebarCreditsText>,
                With<PlayerBottomStanceText>,
            )>,
            Without<TopBarStatusText>,
            Without<SidebarQueueText>,
            Without<BottomInspectorText>,
            Without<StudioStatusText>,
            Without<BattleFeedText>,
        ),
    >,
    mut studio_modal_q: Query<
        (&mut Node, &mut BackgroundColor),
        (
            With<StudioModalRoot>,
            Without<ProductionProgressBarFill>,
            Without<BattleFeedRoot>,
            Without<InGameHudNode>,
            Without<SelectionDragBoxNode>,
            Without<DevToolbarNode>,
            Without<StudioTopDeckNode>,
            Without<Button>,
        ),
    >,
    mut progress_bar_q: Query<
        &mut Node,
        (
            With<ProductionProgressBarFill>,
            Without<StudioModalRoot>,
            Without<BattleFeedRoot>,
            Without<InGameHudNode>,
            Without<SelectionDragBoxNode>,
            Without<DevToolbarNode>,
            Without<StudioTopDeckNode>,
        ),
    >,
    mut feed_root_q: Query<
        &mut Node,
        (
            With<BattleFeedRoot>,
            Without<StudioModalRoot>,
            Without<ProductionProgressBarFill>,
            Without<InGameHudNode>,
            Without<SelectionDragBoxNode>,
            Without<DevToolbarNode>,
            Without<StudioTopDeckNode>,
        ),
    >,
    mut ingame_hud_q: Query<
        (
            &mut Node,
            Option<&TopBarRootNode>,
            Option<&RightSidebarRootNode>,
            Option<&BottomCommandBarRootNode>,
            Option<&PlayerBottomStancePillNode>,
            Option<&SidebarTelemetryStripNode>,
        ),
        (
            With<InGameHudNode>,
            Without<StudioModalRoot>,
            Without<ProductionProgressBarFill>,
            Without<BattleFeedRoot>,
            Without<SelectionDragBoxNode>,
            Without<DevToolbarNode>,
            Without<StudioTopDeckNode>,
        ),
    >,
    mut dev_toolbar_q: Query<
        &mut Node,
        (
            Or<(With<DevToolbarNode>, With<StudioTopDeckNode>)>,
            Without<InGameHudNode>,
            Without<StudioModalRoot>,
            Without<ProductionProgressBarFill>,
            Without<BattleFeedRoot>,
            Without<SelectionDragBoxNode>,
        ),
    >,
    (container_q, studio_mods_q, mut hud_skin_img_q): (
        Query<Entity, With<DynamicButtonsContainer>>,
        Query<Entity, With<StudioModListContainer>>,
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

    let has_mod_skin = crate::input::mod_has_custom_hud_skin(loaded);
    let show_dev_or_unskinned_ui = interaction.dev_mode_enabled || !has_mod_skin;
    let show_ingame_hud = interaction.app_phase == AppPhase::InGame && !interaction.studio_open;
    let top_bar_h = if show_dev_or_unskinned_ui {
        66.0
    } else {
        0.0
    };
    for (
        mut hud_node,
        is_top_bar,
        is_right_sidebar,
        is_bottom_bar,
        is_player_stance_pill,
        is_telemetry_strip,
    ) in &mut ingame_hud_q
    {
        if !show_ingame_hud {
            hud_node.display = Display::None;
            continue;
        }
        if is_top_bar.is_some() {
            hud_node.display = if show_dev_or_unskinned_ui {
                Display::Flex
            } else {
                Display::None
            };
            hud_node.height = Val::Px(top_bar_h);
        } else if is_bottom_bar.is_some() || is_telemetry_strip.is_some() {
            hud_node.display = if show_dev_or_unskinned_ui {
                Display::Flex
            } else {
                Display::None
            };
        } else if is_player_stance_pill.is_some() {
            hud_node.display =
                if !show_dev_or_unskinned_ui && !interaction.selected_ids.is_empty() {
                    Display::Flex
                } else {
                    Display::None
                };
        } else if is_right_sidebar.is_some() {
            hud_node.display = Display::Flex;
            hud_node.top = Val::Px(top_bar_h);
        } else {
            hud_node.display = Display::Flex;
        }
    }
    for mut dev_bar in &mut dev_toolbar_q {
        dev_bar.display = if show_dev_or_unskinned_ui {
            Display::Flex
        } else {
            Display::None
        };
    }

    // Apply faction-specific RA2 HUD skin textures (credits.png, top.png, optbtn.png, diplobtn.png, radar.png)
    let active_fac_id = world
        .players
        .get(&pid)
        .map(|e| e.faction.as_str())
        .unwrap_or("");
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
                let qual = crate::input::qualify_mod_asset_rel_path(loaded, rel);
                if let Some(h) = super::menu_screens::load_ui_image(asset_server.as_deref(), &qual)
                {
                    if img_node.image != h {
                        img_node.image = h;
                    }
                }
            }
        }
    }

    let player_credits = world.players.get(&pid).map(|e| e.credits).unwrap_or(0);
    for (mut txt, is_hdr, is_tab6, is_cred_txt, is_stance_txt) in &mut studio_hdr_text_q {
        if is_hdr.is_some() {
            let hdr_str = if interaction.dev_mode_enabled {
                "IRONLOCK // RUST-RTS 3D ENGINE STUDIO & DEVELOPER SANDBOX (rust-rts.bls-isp.net)"
                    .to_string()
            } else {
                format!(
                    "{} — COMMAND CENTER (v{})",
                    loaded.manifest.name.to_uppercase(),
                    loaded.manifest.version
                )
            };
            **txt = studio_layout::sanitize_ui_text(&hdr_str);
        }
        if is_tab6.is_some() {
            let tab6_str = if interaction.dev_mode_enabled {
                "6. STUDIO & 3D ENCYCLOPEDIA"
            } else {
                "6. UNIT ENCYCLOPEDIA"
            };
            **txt = studio_layout::sanitize_ui_text(tab6_str);
        }
        if is_cred_txt.is_some() {
            **txt = format!("$ {:05}", player_credits);
        }
        if is_stance_txt.is_some() && !interaction.selected_ids.is_empty() {
            let sel_line = build_bottom_inspector_string(world, loaded, &interaction)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            **txt = studio_layout::sanitize_ui_text(&sel_line);
        }
    }

    // Toggle Studio modal visibility & adaptive glassmorphic background
    if let Ok((mut modal_node, mut modal_bg)) = studio_modal_q.get_single_mut() {
        modal_node.display = if interaction.studio_open {
            Display::Flex
        } else {
            Display::None
        };
        if show_dev_or_unskinned_ui {
            modal_node.padding = UiRect::all(Val::Px(18.0));
            modal_node.row_gap = Val::Px(10.0);
        } else {
            modal_node.padding = UiRect::all(Val::Px(0.0));
            modal_node.row_gap = Val::Px(0.0);
        }
        *modal_bg = if interaction.app_phase == AppPhase::LoadingScreen {
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.98))
        } else if interaction.app_phase == AppPhase::MainMenu
            && interaction.active_menu_screen == MenuScreen::Root
        {
            BackgroundColor(Color::srgba(0.01, 0.02, 0.05, 0.10))
        } else {
            BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.72))
        };
    }

    for mut st_text in &mut studio_status_q {
        **st_text = studio_layout::sanitize_ui_text(&interaction.studio_status_message);
    }

    // Update EVA Battle Feed, Mission Objective, Chat Overlay, [F9] Tech-Tree Encyclopedia & [Tab] Match Scoreboard Banner
    let show_scoreboard = interaction.scoreboard_open || world.winner_player_id.is_some();
    let show_tech_tree = interaction.tech_tree_open;
    let show_dev_console = interaction.dev_console_open;
    let has_objective = world.mission_objective.is_some();
    let has_chat = interaction.chat_input_open || !interaction.chat_messages.is_empty();
    let has_campaign_briefing = interaction.active_campaign_id.is_some();
    if let Ok(mut feed_node) = feed_root_q.get_single_mut() {
        feed_node.top = Val::Px(top_bar_h + 6.0);
        feed_node.display = if (interaction.battle_feed.is_empty()
            && !show_scoreboard
            && !show_tech_tree
            && !show_dev_console
            && !has_objective
            && !has_chat
            && !has_campaign_briefing)
            || interaction.studio_open
        {
            Display::None
        } else {
            Display::Flex
        };
    }
    if let Ok(mut feed_text) = feed_text_q.get_single_mut() {
        let mut sections: Vec<String> = Vec::new();
        if show_dev_console {
            let p = &world.last_tick_profile;
            let sh = world.subsystem_hashes();
            let mode_tag = if interaction.is_multiplayer {
                "READ-ONLY MULTIPLAYER PROFILER"
            } else {
                "FULL INTERACTIVE SANDBOX"
            };
            sections.push(format!(
                "=== DEV-CONSOLE & TICK-PROFILER [F9] ({mode_tag}) ==="
            ));
            sections.push(format!(
                "Tick {} | Entities: {} | Projectiles: {} | PathCacheHits: {} | Total: {}us (Cmd {}us, Move {}us, Combat {}us, Proj {}us, Trig/FoW {}us)",
                world.tick,
                p.entity_count,
                p.projectile_count,
                p.path_cache_hits,
                p.total_us,
                p.commands_us,
                p.movement_us,
                p.combat_us,
                p.projectiles_us,
                p.triggers_fow_us
            ));
            sections.push(format!(
                "Subsystems: econ=0x{:08x} tf=0x{:08x} combat=0x{:08x} res=0x{:08x} proj=0x{:08x} trig=0x{:08x}",
                sh.economy as u32,
                sh.entities_transform as u32,
                sh.entities_combat as u32,
                sh.resources_and_mines as u32,
                sh.projectiles as u32,
                sh.triggers as u32
            ));
            let gf = &interaction.debug_gizmo_flags;
            sections.push(format!(
                "3D Debug Gizmos (type `gizmo <paths|weapons|vision|triggers|grid|all>`): paths={} weapons={} vision={} triggers={} grid={}",
                gf.show_paths, gf.show_weapon_ranges, gf.show_vision_and_stealth, gf.show_trigger_zones, gf.show_blocked_tiles
            ));
            for line in &interaction.dev_console_history {
                sections.push(line.clone());
            }
            sections.push(format!(
                "DEV [spawn|credits|var|trigger|heal|kill|instant_build|reveal|step|gizmo] > {}_",
                interaction.dev_console_input
            ));
        }
        if show_tech_tree {
            let faction_id = world
                .players
                .get(&pid)
                .map(|e| e.faction.as_str())
                .or_else(|| loaded.factions.keys().next().map(|s| s.as_str()))
                .unwrap_or("");
            let avail_b = world.available_buildings_for_player(pid, loaded);
            let avail_u = world.available_units_for_player(pid, loaded);
            let mut owned_b: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for ent in world.entities.values() {
                if ent.player_id == pid && ent.health > 0.0 && ent.construction_progress >= 1.0 {
                    if let EntityKind::Building { building_id } = &ent.kind {
                        owned_b.insert(building_id.clone());
                    }
                }
            }
            sections.push(format!(
                "=== TECH-TREE & UNIT ENCYCLOPEDIA [F9] â€” FACTION: {} ===",
                faction_id.to_ascii_uppercase()
            ));
            sections.push("--- STRUCTURES & PREREQUISITE TREE ---".to_string());
            for (b_id, b_def) in &loaded.buildings {
                if b_def.faction != faction_id {
                    continue;
                }
                let st = if owned_b.contains(b_id) {
                    "BUILT â˜…"
                } else if avail_b.contains(b_id) {
                    "UNLOCKED âœ”"
                } else {
                    "LOCKED ðŸ”’"
                };
                let reqs = if b_def.prerequisites.is_empty() {
                    "None".to_string()
                } else {
                    b_def.prerequisites.join(" + ")
                };
                let sp_info = if b_def.support_powers.is_empty() {
                    String::new()
                } else {
                    let names: Vec<&str> = b_def
                        .support_powers
                        .iter()
                        .map(|sp| sp.name.as_str())
                        .collect();
                    format!(" | Support Powers: [{}]", names.join(", "))
                };
                sections.push(format!(
                    "â€¢ [{}] {} (${}, {:+}P, {}x{}) â€” Requires: {}{}",
                    st,
                    b_def.name,
                    b_def.cost,
                    b_def.power_delta,
                    b_def.footprint[0],
                    b_def.footprint[1],
                    reqs,
                    sp_info
                ));
            }
            sections.push("--- UNITS & COMBAT ENCYCLOPEDIA ---".to_string());
            for (u_id, u_def) in &loaded.units {
                if u_def.faction != faction_id {
                    continue;
                }
                let st = if avail_u.contains(u_id) {
                    "UNLOCKED âœ”"
                } else {
                    "LOCKED ðŸ”’"
                };
                let reqs = if u_def.prerequisites.is_empty() {
                    "Factory".to_string()
                } else {
                    u_def.prerequisites.join(" + ")
                };
                let wp_info = u_def
                    .primary_weapon
                    .as_ref()
                    .and_then(|wid| loaded.weapons.get(wid))
                    .map(|w| format!("DMG {:.0} (Rng {:.0})", w.damage, w.range))
                    .unwrap_or_else(|| "Unarmed/Support".to_string());
                sections.push(format!(
                    "â€¢ [{}] {} (${}, {:.0} HP, {:?} Armor, Spd {:.1}, {}) â€” Requires: {}",
                    st,
                    u_def.name,
                    u_def.cost,
                    u_def.health,
                    u_def.armor,
                    u_def.speed,
                    wp_info,
                    reqs
                ));
            }
        }
        if let (Some(c_id), Some(m_id)) = (
            &interaction.active_campaign_id,
            &interaction.active_mission_id,
        ) {
            if let Some(camp) = loaded.campaigns.get(c_id) {
                if let Some(miss) = camp.missions.iter().find(|m| &m.id == m_id) {
                    sections.push(format!(
                        "â˜… CAMPAIGN [{}]: {} â€” {}",
                        camp.display_name, miss.display_name, miss.briefing
                    ));
                }
            }
        }
        if let Some(obj) = &world.mission_objective {
            sections.push(format!("â˜… MISSION OBJECTIVE: {obj}"));
        }
        if show_scoreboard {
            let banner_hdr = match world.winner_player_id {
                Some(w) if w == pid => "=== VICTORY! POST-GAME MATCH STATISTICS [TAB] ===".to_string(),
                Some(w) => format!("=== PLAYER {w} WINS! POST-GAME STATISTICS [TAB] ==="),
                None => "=== LIVE MATCH SCOREBOARD & DIPLOMACY [TAB] ===".to_string(),
            };
            sections.push(banner_hdr);
            for (&p_id, econ) in &world.players {
                let dip_tag = if p_id == pid {
                    "YOU"
                } else if !world.are_hostile(pid, p_id) {
                    "ALLY"
                } else {
                    "HOSTILE"
                };
                let army_val = world.player_army_value(p_id, loaded);
                let apm_val = world.player_apm(p_id);
                sections.push(format!(
                    "P{p_id} ({}) [{}] | Credits: ${} (Harvested: ${}) | Army: ${army_val} | APM: {apm_val} | Units Built/Killed/Lost: {}/{}/{} | Bldgs Built/Destroyed/Lost: {}/{}/{}",
                    econ.faction,
                    dip_tag,
                    econ.credits,
                    econ.credits_harvested,
                    econ.units_built,
                    econ.units_killed,
                    econ.units_lost,
                    econ.buildings_built,
                    econ.buildings_destroyed,
                    econ.buildings_lost
                ));
            }
        }
        for (msg, _) in &interaction.battle_feed {
            sections.push(msg.clone());
        }
        for (chat_msg, _) in &interaction.chat_messages {
            sections.push(format!("ðŸ’¬ {chat_msg}"));
        }
        if interaction.chat_input_open {
            sections.push(format!("ðŸ’¬ CHAT [ENTER]: {}_", interaction.chat_input_buffer));
        }
        if !sections.is_empty() {
            **feed_text = studio_layout::sanitize_ui_text(&sections.join("\n"));
        }
    }

    // 1. Update Top Bar Text
    if let Ok(mut top_text) = top_text_q.get_single_mut() {
        if let Some(econ) = world.players.get(&pid) {
            let faction_name = loaded
                .factions
                .get(&econ.faction)
                .map(|f| f.name.as_str())
                .unwrap_or(&econ.faction);
            let power_status = if econ.power_sabotage_timer > 0.0 {
                "SABOTAGED!"
            } else if econ.is_low_power() {
                "LOW POWER!"
            } else {
                "ONLINE"
            };
            let vet_tag = if econ.veteran_production_unlocked {
                " [VET*]"
            } else {
                ""
            };
            let ai_status = if interaction.is_multiplayer {
                "NET".to_string()
            } else if interaction.ai_enabled {
                format!(
                    "AI: {}/{}",
                    interaction.ai_difficulty.label(),
                    interaction.ai_personality.label()
                )
            } else {
                "AI: OFF".to_string()
            };
            let fog_status = if interaction.fog_of_war_enabled {
                "FOG: ON"
            } else {
                "FOG: OFF"
            };
            let speed_status = if interaction.game_paused {
                "PAUSED".to_string()
            } else {
                format!("{:.1}x", interaction.sim_speed_multiplier)
            };
            let victory = match world.winner_player_id {
                Some(w) if w == pid => " | *** VICTORY! ***".to_string(),
                Some(w) => format!(" | *** PLAYER {w} WINS! ***"),
                None => String::new(),
            };

            let supply_str = if econ.supply_cap > 0 || econ.supply_used > 0 {
                format!(" | SUP {}/{}", econ.supply_used, econ.supply_cap)
            } else {
                String::new()
            };
            let follow_str = if interaction.follow_entity_id.is_some() {
                " | [FOLLOW CAM]"
            } else {
                ""
            };
            let refine_mult = world.player_ore_refine_multiplier(pid, loaded);
            let refine_str = if refine_mult > 1.01 {
                format!(" (+{:.0}%)", (refine_mult - 1.0) * 100.0)
            } else {
                String::new()
            };
            let cmd_rank_str = format!(
                "[*]R{} ({:.0}XP/{}PP)",
                econ.commander_rank, econ.commander_xp, econ.promotion_points
            );
            let sw_warnings = world.active_superweapon_warnings(loaded);
            let sw_str = if sw_warnings.is_empty() {
                String::new()
            } else {
                let parts: Vec<String> = sw_warnings
                    .into_iter()
                    .take(2)
                    .map(|(owner, name, cd)| {
                        if cd <= 0.0 {
                            format!("P{owner} {name} [READY]")
                        } else {
                            format!("P{owner} {name} [{cd:.0}s]")
                        }
                    })
                    .collect();
                format!(" | [!] SW: {}", parts.join(", "))
            };

            let raw_top = if interaction.dev_mode_enabled {
                format!(
                    "[DEV:{}] P{} ({}{} | {}) | ${}{} | PWR +{}/-{} ({}){} | FORM: {} [J] | {} | {} | {} | {} [L] | {}{} | Tick {}{}{}",
                    loaded.manifest.id.to_ascii_uppercase(),
                    pid,
                    faction_name,
                    vet_tag,
                    cmd_rank_str,
                    econ.credits,
                    refine_str,
                    econ.power_produced,
                    econ.power_consumed,
                    power_status,
                    supply_str,
                    econ.formation.label(),
                    ai_status,
                    world.game_mode.label(),
                    fog_status,
                    interaction.environment_preset.label(),
                    speed_status,
                    follow_str,
                    world.tick,
                    sw_str,
                    victory
                )
            } else {
                format!(
                    "[{}]  CREDITS: ${}{}  |  PWR: +{}/-{} ({}){}  |  RANK: {}{}  |  SPEED: {}{}{}{}",
                    faction_name.to_uppercase(),
                    econ.credits,
                    refine_str,
                    econ.power_produced,
                    econ.power_consumed,
                    power_status,
                    supply_str,
                    cmd_rank_str,
                    vet_tag,
                    speed_status,
                    follow_str,
                    sw_str,
                    victory
                )
            };
            **top_text = studio_layout::sanitize_ui_text(&raw_top);
        }
    }

    // 2. Update Sidebar Queue & Placement status + Animated Production Progress Bar
    if let Ok(mut q_text) = queue_text_q.get_single_mut() {
        let mut active_queues = Vec::new();
        let mut primary_progress_pct = 0.0_f32;
        if let Some(econ) = world.players.get(&pid) {
            if let Some((b_id, rem_secs, total_secs)) = &econ.building_queue {
                let pct_f =
                    ((1.0 - rem_secs / total_secs.max(0.1)) * 100.0).clamp(0.0, 100.0);
                primary_progress_pct = pct_f;
                let b_name = loaded
                    .buildings
                    .get(b_id)
                    .map(|b| b.name.as_str())
                    .unwrap_or(b_id);
                active_queues.push(format!("Pre-Build {b_name} ({:.0}%)", pct_f));
            } else if let Some(ready_id) = &econ.ready_building_id {
                primary_progress_pct = 100.0;
                let b_name = loaded
                    .buildings
                    .get(ready_id)
                    .map(|b| b.name.as_str())
                    .unwrap_or(ready_id);
                active_queues.push(format!("Pre-Build {b_name} [READY]"));
            }
            if let Some((up_id, rem_secs, total_secs)) = &econ.active_upgrade {
                let pct_f =
                    ((1.0 - rem_secs / total_secs.max(0.1)) * 100.0).clamp(0.0, 100.0);
                if primary_progress_pct == 0.0 {
                    primary_progress_pct = pct_f;
                }
                let up_name = loaded
                    .upgrades
                    .get(up_id)
                    .map(|u| u.name.as_str())
                    .unwrap_or(up_id);
                active_queues.push(format!("Upgrade {up_name} ({:.0}%)", pct_f));
            }
        }
        for ent in world.entities.values() {
            if ent.player_id == pid {
                if let EntityKind::Building { building_id } = &ent.kind {
                    if ent.construction_progress < 1.0 {
                        let c_pct = (ent.construction_progress * 100.0).clamp(5.0, 99.0);
                        if primary_progress_pct == 0.0 {
                            primary_progress_pct = c_pct;
                        }
                        let b_name = loaded
                            .buildings
                            .get(building_id)
                            .map(|b| b.name.as_str())
                            .unwrap_or(building_id);
                        active_queues.push(format!("Building {b_name} ({:.0}%)", c_pct));
                    } else if let Some((target_b_id, rem_secs, total_secs)) = &ent.upgrading_to {
                        let u_pct = ((1.0 - rem_secs / total_secs.max(0.1)) * 100.0).clamp(1.0, 99.0);
                        if primary_progress_pct == 0.0 {
                            primary_progress_pct = u_pct;
                        }
                        let t_name = loaded
                            .buildings
                            .get(target_b_id)
                            .map(|b| b.name.as_str())
                            .unwrap_or(target_b_id);
                        active_queues.push(format!("Upgrading -> {t_name} ({:.0}%)", u_pct));
                    }
                }
            }
            if ent.player_id == pid && !ent.production_queue.is_empty() {
                let item = &ent.production_queue[0];
                let pct_f = ((1.0 - item.remaining_secs / item.total_secs.max(0.1)) * 100.0)
                    .clamp(0.0, 100.0);
                if primary_progress_pct == 0.0 {
                    primary_progress_pct = pct_f;
                }
                let pct = pct_f as u32;
                let u_name = loaded
                    .units
                    .get(&item.unit_id)
                    .map(|u| u.name.as_str())
                    .unwrap_or(&item.unit_id);
                let total_in_q = ent.production_queue.len();
                if total_in_q > 1 {
                    active_queues.push(format!("{u_name} ({pct}%, +{total_in_q})"));
                } else {
                    active_queues.push(format!("{u_name} ({pct}%)"));
                }
            }
        }
        if let Ok(mut bar_node) = progress_bar_q.get_single_mut() {
            bar_node.width = Val::Percent(primary_progress_pct);
        }
        let queue_line = if active_queues.is_empty() {
            "Production: Idle".to_string()
        } else {
            format!("Training: {}", active_queues.join(", "))
        };
        let placing_line = match &interaction.placing_building_id {
            Some(b_id) => {
                let b_name = loaded
                    .buildings
                    .get(b_id)
                    .map(|b| b.name.as_str())
                    .unwrap_or(b_id);
                format!("Placing: {b_name} (LMB Place / RMB Cancel)")
            }
            None => {
                if interaction.attack_move_mode {
                    format!(
                        "[ATTACK-MOVE (X): RMB CLICK TARGET] | Selected: {}",
                        interaction.selected_ids.len()
                    )
                } else {
                    format!("Selected: {} Entity(s)", interaction.selected_ids.len())
                }
            }
        };
        **q_text = studio_layout::sanitize_ui_text(&format!("{placing_line}\n{queue_line}"));
    }

    if let Ok(mut bot_text) = bottom_text_q.get_single_mut() {
        **bot_text =
            studio_layout::sanitize_ui_text(&build_bottom_inspector_string(world, loaded, &interaction));
    }
    update_sidebar_and_studio_lists(
        &mut commands,
        asset_server.as_deref(),
        &asset_root,
        world,
        loaded,
        &interaction,
        &mut last_sig,
        &container_q,
        &studio_mods_q,
    );
}
