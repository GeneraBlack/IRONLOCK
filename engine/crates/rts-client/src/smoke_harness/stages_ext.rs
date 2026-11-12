use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, MenuScreen, UiAssetCache};
use crate::plugins::NetRuntimeResource;
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, MapTerrainDirty,
    MapTerrainDirtyRects, PreviewStageCamera, SimVisualEntity,
};
use crate::studio::{self, StudioState};
use crate::ui::{LastSidebarSignature, PlayerSubWindowStack};
use crate::vfx::VfxTrackerState;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_network::replay::ReplayFile;
use rts_sdk::loader::LoadedMod;
use rts_sdk::manifest::{EconomyModel, HudLayoutPreset};
use rts_simulation::command::{GameCommand, QueuedOrder};
use rts_simulation::world::{
    EnergyPoolState, EntityKind, HeroInstanceState, InventorySlotItem, LocalResearchItem,
    ProductionFacilityState, ProductionItem, SimWorld, TRAIT_ENERGY, TRAIT_HERO, TRAIT_PRODUCTION,
};

#[allow(clippy::too_many_arguments)]
pub fn setup_stage_09_to_12(
    frame: u32,
    mod_res: &ModResource,
    sim_res: &mut SimWorldResource,
    net_res: &mut NetRuntimeResource,
    interaction: &mut ClientInteractionState,
    studio_state: &mut Option<ResMut<StudioState>>,
    cached_fow: &mut CachedFogVisibility,
    subwindows: &mut PlayerSubWindowStack,
    last_sig: &mut LastSidebarSignature,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
) {
    match frame {
        300 => {
            interaction.dev_mode_enabled = true;
            interaction.active_sidebar_tab = crate::input::ProductionSidebarTab::Vehicles;
            interaction.dev_console_open = true;
            interaction.dev_console_history =
                vec!["> status -> Deterministic Fixed-Point Sim online (20 Hz)".to_string()];
            interaction.dev_console_input = "spawn prism_tank 2 1".to_string();
            interaction.debug_gizmo_flags.show_paths = true;
            interaction.debug_gizmo_flags.show_weapon_ranges = true;
            interaction.debug_gizmo_flags.show_vision_and_stealth = true;
            interaction.debug_gizmo_flags.show_trigger_zones = true;
            interaction.debug_gizmo_flags.show_blocked_tiles = true;
            last_sig.sidebar_sig.clear();
        }
        330 => {
            interaction.dev_mode_enabled = false;
            interaction.dev_console_open = false;
            interaction.debug_gizmo_flags = Default::default();
            subwindows.close_all();
            let mut rep = ReplayFile::new(&mod_res.0);
            rep.final_tick = (sim_res.0.tick + 1200).max(1200);
            net_res.active_replay_playback = Some(rep);
            interaction.replay_active = true;
            interaction.replay_total_ticks = 1200;
            interaction.observer_pov_player_id = 255;
            interaction.spectator_all_vision = true;
            interaction.fog_of_war_enabled = false;
            cached_fow.reset();
            last_sig.sidebar_sig.clear();
        }
        360 => {
            net_res.active_replay_playback = None;
            interaction.replay_active = false;
            for _ in 0..85 {
                sim_res.0.step(&mod_res.0);
            }
            sim_res.0.winner_player_id = Some(1);
            net_res.post_game_triggered_winner = Some(1);
            interaction.app_phase = AppPhase::PostGameScore;
            interaction.active_menu_screen = MenuScreen::PostGameScore;
            interaction.post_game_tab = 1;
            interaction.menu_open = true;
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
        }
        390 => {
            sim_res.0.winner_player_id = None;
            net_res.post_game_triggered_winner = None;
            interaction.app_phase = AppPhase::InGame;
            interaction.menu_open = false;
            interaction.studio_open = true;
            interaction.studio_playtest_active = false;
            interaction.fog_of_war_enabled = false;
            cached_fow.reset();

            let selected_unit = sim_res
                .0
                .entities
                .values()
                .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
                .map(|e| (e.id, e.position));
            interaction.selected_ids.clear();
            if let Some((uid, upos)) = selected_unit {
                interaction.selected_ids.insert(uid);
                if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
                    rig.distance = 26.0;
                    rig.target_distance = 26.0;
                    rig.snap_to(Vec3::new(upos[0], upos[1], upos[2]));
                    *tf = rig.compute_transform();
                }
            }
            if let Some(ref mut st) = studio_state {
                st.active_tool = studio::world_builder::WorldBuilderTool::SelectAndMove;
                st.browser_category = studio::asset_browser::AssetBrowserCategory::CommanderTrees;
                if let Some(first_tree_id) = mod_res.0.commander_trees.keys().next().cloned() {
                    st.inspect_target = studio::SdkInspectTarget::CommanderTree(first_tree_id);
                } else if let Some(first_fac_id) = mod_res.0.factions.keys().next().cloned() {
                    st.inspect_target = studio::SdkInspectTarget::Faction(first_fac_id);
                }
                st.spline_tools.mode =
                    studio::spline_and_prefab_tools::ActiveAdvancedTerrainMode::SplineAuthoring;
                st.spline_tools.spline_kind = studio::spline_and_prefab_tools::SplineKind::Road;
                let base_y = sim_res.0.grid.sample_world_height(30.0, 30.0);
                st.spline_tools.spline_points = vec![
                    [26.0, base_y, 27.0],
                    [30.0, base_y, 29.5],
                    [34.5, base_y, 31.0],
                    [38.5, base_y, 34.0],
                ];
                studio::dock_layout::focus_or_open_tab(
                    &mut st.dock_state,
                    studio::dock_layout::StudioTab::PropertyInspector,
                );
                studio::dock_layout::focus_or_open_tab(
                    &mut st.dock_state,
                    studio::dock_layout::StudioTab::WorldBuilder,
                );
            }
            last_sig.studio_sig.clear();
            last_sig.sidebar_sig.clear();
        }
        _ => {}
    }
}

/// Configures Stage `13` (`13_orbital_strategic_zoom_and_bottom_hud.png`):
/// - Loads `sdk-showcase-heroes-ages-mutators` (or enriches the active mod if absent)
/// - Activates **`ModernBottomCommandCard`** (`HudLayoutPreset::ModernBottomCommandCard`)
/// - Configures Lv.4 Hero with Mana (`165 / 225`), `+2` Skill Points, golden Auto-Cast ability, and 6-Slot Inventory
/// - Configures Era Progression (`Age I` -> `[Advance -> Age II]`), live **Streaming Economy** (`+68/s -54/s 94% EFF`), and parallel production/local-research queues
/// - Issues multi-waypoint `[Shift]` queued orders across P1 strike groups and zooms camera to Orbital Strategic Zoom (`295m`)
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn setup_stage_13_orbital_strategic_zoom_and_bottom_hud(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    mod_res: &mut ModResource,
    sim_res: &mut SimWorldResource,
    interaction: &mut ClientInteractionState,
    ui_cache: Option<&mut UiAssetCache>,
    keys: Option<&mut ButtonInput<KeyCode>>,
    cached_fow: &mut CachedFogVisibility,
    vfx_state: &mut VfxTrackerState,
    subwindows: &mut PlayerSubWindowStack,
    last_sig: &mut LastSidebarSignature,
    terrain_dirty: &mut Option<ResMut<MapTerrainDirty>>,
    terrain_dirty_rects: &mut Option<ResMut<MapTerrainDirtyRects>>,
    env_q: &Query<Entity, With<MapEnvironmentEntity>>,
    visual_q: &Query<(Entity, &SimVisualEntity, Option<&Children>)>,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
) {
    subwindows.close_all();
    interaction.studio_open = false;
    interaction.menu_open = false;
    interaction.dev_mode_enabled = false;
    interaction.dev_console_open = false;
    interaction.app_phase = AppPhase::InGame;
    interaction.fog_of_war_enabled = false;
    interaction.hud_layout_override = Some(HudLayoutPreset::ModernBottomCommandCard);
    interaction.virtual_shift_preview = true;
    if let Some(k) = keys {
        k.press(KeyCode::ShiftLeft);
    }

    // Load `mods/sdk-showcase-heroes-ages-mutators` when present in workspace so genuine Hero/Era/Streaming schemas drive HUD + 3D!
    let showcase_candidates = [
        LoadedMod::resolve_mod_dir("mods/sdk-showcase-heroes-ages-mutators"),
        LoadedMod::resolve_mod_dir("sdk-showcase-heroes-ages-mutators"),
        LoadedMod::resolve_mod_dir("rust-ra2")
            .parent()
            .map(|p| p.join("sdk-showcase-heroes-ages-mutators"))
            .unwrap_or_default(),
    ];
    if let Some(showcase_dir) = showcase_candidates
        .into_iter()
        .find(|d| d.join("mod.toml").exists())
    {
        if let Ok(loaded_showcase) = LoadedMod::load_from_dir(&showcase_dir) {
            for ent in env_q {
                commands.entity(ent).despawn_recursive();
            }
            for (ent, _, _) in visual_q {
                commands.entity(ent).despawn_recursive();
            }
            mod_res.0 = loaded_showcase;
            interaction.active_map_id = mod_res.0.entry_map.id.clone();
            sim_res.0 = SimWorld::from_mod(&mod_res.0);
            if let Some(uc) = ui_cache {
                uc.sync_loaded_mod(&mod_res.0);
            }
            vfx_state.reset();
            spawn_map_environment_entities(commands, meshes, materials, &mod_res.0, &sim_res.0);
            if let Some(ref mut td) = terrain_dirty {
                td.0 = true;
            }
            if let Some(ref mut tr) = terrain_dirty_rects {
                tr.mark_all();
            }
        }
    } else {
        mod_res.0.manifest.rules.economy_model = EconomyModel::Streaming;
    }

    // Ensure P1 has prerequisite buildings (`vanguard_barracks`, `arcane_smithy`) for `[Advance -> Age II]`
    for (bid, tile) in [("vanguard_barracks", [12, 6]), ("arcane_smithy", [15, 6])] {
        if mod_res.0.buildings.contains_key(bid) {
            let _ = sim_res.0.spawn_building(1, bid, tile, &mod_res.0, false);
        }
    }

    cached_fow.reset();

    // Ensure P1 has an active Hero with Level 4, Mana, Skill Points, Auto-Cast & 6-Slot Relics
    let mut hero_id_opt = sim_res
        .0
        .entities
        .values()
        .find(|e| e.player_id == 1 && sim_res.0.trait_stores.heroes.contains_key(&e.id))
        .map(|e| e.id);

    if hero_id_opt.is_none() {
        if let Some(hero_uid) = mod_res
            .0
            .units
            .values()
            .find(|u| u.hero_trait.is_some())
            .map(|u| u.id.clone())
        {
            hero_id_opt = sim_res.0.spawn_unit(1, &hero_uid, [14, 14], &mod_res.0);
        } else {
            hero_id_opt = sim_res
                .0
                .entities
                .values()
                .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
                .map(|e| e.id);
        }
    }

    let item_ids: Vec<String> = mod_res.0.items.keys().take(4).cloned().collect();
    if let Some(hid) = hero_id_opt {
        let mut learned = std::collections::BTreeMap::new();
        let mut auto_idx = 0usize;
        if let Some(ent) = sim_res.0.entities.get_mut(&hid) {
            ent.experience = 295.0;
            ent.trait_flags |= TRAIT_HERO | TRAIT_ENERGY;
            if let EntityKind::Unit { ref unit_id } = ent.kind {
                if let Some(udef) = mod_res.0.units.get(unit_id) {
                    for (idx, ab) in udef.abilities.iter().enumerate() {
                        if idx == 0 {
                            learned.insert(ab.id.clone(), 2);
                            auto_idx = idx;
                        } else if idx == 1 {
                            learned.insert(ab.id.clone(), 1);
                        }
                    }
                }
            }
            ent.auto_cast_abilities.insert(auto_idx);
        }
        let mut inv = vec![None; 6];
        for (slot, iid) in item_ids.iter().enumerate().take(6) {
            let charges = mod_res
                .0
                .items
                .get(iid)
                .map(|it| it.max_charges.max(1))
                .unwrap_or(1);
            inv[slot] = Some(InventorySlotItem {
                item_id: iid.clone(),
                charges,
            });
        }
        sim_res.0.trait_stores.heroes.insert(
            hid,
            HeroInstanceState {
                level: 4,
                skill_points: 2,
                learned_skills: learned,
                inventory: inv,
            },
        );
        sim_res.0.trait_stores.energy_pools.insert(
            hid,
            EnergyPoolState {
                energy: 168.0,
                max_energy: 225.0,
                regen_per_sec: 4.5,
            },
        );
        interaction.selected_ids.clear();
        interaction.selected_ids.insert(hid);
    }

    // Spawn additional P1 & P2 squads/vehicles spread across the map for rich Strategic Icons + Shift-Queued Paths
    let p1_combat_uids: Vec<String> = mod_res
        .0
        .units
        .values()
        .filter(|u| u.hero_trait.is_none() && u.harvest_capacity == 0)
        .take(3)
        .map(|u| u.id.clone())
        .collect();
    let tile_w = mod_res.0.entry_map.width.max(32) as i32;
    let tile_h = mod_res.0.entry_map.height.max(32) as i32;
    let map_w = (tile_w as f32) * mod_res.0.entry_map.tile_size;
    let map_h = (tile_h as f32) * mod_res.0.entry_map.tile_size;
    let mut strike_group_a = Vec::new();
    let mut strike_group_b = Vec::new();
    for (i, uid) in p1_combat_uids.iter().cycle().take(8).enumerate() {
        let tx = (tile_w / 4) + ((i as i32) % 4) * 2;
        let tz = (tile_h / 3) + ((i as i32) / 4) * 2;
        if let Some(eid) = sim_res.0.spawn_unit(1, uid, [tx, tz], &mod_res.0) {
            if i < 4 {
                strike_group_a.push(eid);
            } else {
                strike_group_b.push(eid);
            }
        }
        let ex = (tile_w * 3 / 4) - ((i as i32) % 4) * 2;
        let ez = (tile_h * 2 / 3) - ((i as i32) / 4) * 2;
        let _ = sim_res.0.spawn_unit(2, uid, [ex, ez], &mod_res.0);
    }

    if !strike_group_a.is_empty() {
        interaction.control_groups[0] = strike_group_a.iter().copied().collect();
        let wp1 = [map_w * 0.44, 0.0, map_h * 0.36];
        let wp2 = [map_w * 0.58, 0.0, map_h * 0.50];
        let wp3 = [map_w * 0.72, 0.0, map_h * 0.66];
        sim_res.0.push_command(GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: strike_group_a.clone(),
            target_pos: wp1,
        });
        sim_res.0.push_command(GameCommand::EnqueueOrder {
            player_id: 1,
            entity_ids: strike_group_a.clone(),
            order: QueuedOrder::Move {
                target_pos: wp2,
                facing_radians: None,
            },
        });
        sim_res.0.push_command(GameCommand::EnqueueOrder {
            player_id: 1,
            entity_ids: strike_group_a,
            order: QueuedOrder::AttackMove {
                target_pos: wp3,
                facing_radians: None,
            },
        });
    }
    if !strike_group_b.is_empty() {
        interaction.control_groups[1] = strike_group_b.iter().copied().collect();
        let wp1 = [map_w * 0.36, 0.0, map_h * 0.56];
        let wp2 = [map_w * 0.56, 0.0, map_h * 0.70];
        sim_res.0.push_command(GameCommand::MoveUnits {
            player_id: 1,
            entity_ids: strike_group_b.clone(),
            target_pos: wp1,
        });
        sim_res.0.push_command(GameCommand::EnqueueOrder {
            player_id: 1,
            entity_ids: strike_group_b,
            order: QueuedOrder::AttackMove {
                target_pos: wp2,
                facing_radians: None,
            },
        });
    }
    sim_res.0.step(&mod_res.0);

    if let Some(first_uid) = p1_combat_uids.first().cloned() {
        let mut p1_buildings: Vec<u64> = sim_res
            .0
            .entities
            .values()
            .filter(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Building { .. }))
            .map(|e| e.id)
            .collect();
        p1_buildings.sort_unstable();
        let prod_bid = p1_buildings
            .iter()
            .copied()
            .find(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .is_some_and(|e| matches!(&e.kind, EntityKind::Building { building_id } if building_id == "vanguard_barracks"))
            })
            .or_else(|| p1_buildings.first().copied());
        if let Some(bid) = prod_bid {
            if let Some(b_ent) = sim_res.0.entities.get_mut(&bid) {
                b_ent.trait_flags |= TRAIT_PRODUCTION;
                b_ent.production_queue.clear();
                b_ent.production_queue.push(ProductionItem {
                    unit_id: first_uid,
                    remaining_secs: 6.8,
                    total_secs: 10.0,
                    on_hold: false,
                });
            }
            let upg_id = mod_res
                .0
                .upgrades
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "solar_tempering".to_string());
            let mut pf = ProductionFacilityState::default();
            pf.local_research_queue.push_back(LocalResearchItem {
                upgrade_id: upg_id,
                remaining_secs: 7.4,
                total_secs: 11.0,
                paid_fraction: 0.36,
                is_era_advance: false,
            });
            sim_res.0.trait_stores.production.insert(bid, pf);
        }
    }
    maintain_stage_13_streaming_hud_metrics(mod_res, sim_res);

    // Position camera at Orbital Strategic Zoom (`225m`) filling the `64x64` battlefield crisply
    if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
        rig.map_max = Vec2::new(map_w, map_h);
        rig.reset_orientation();
        rig.set_zoom_distance(225.0);
        rig.distance = 225.0;
        rig.target_distance = 225.0;
        let cx = map_w * 0.50;
        let cz = map_h * 0.50;
        let cy = sim_res.0.grid.sample_world_height(cx, cz);
        rig.snap_to(Vec3::new(cx, cy, cz));
        *tf = rig.compute_transform();
    }

    last_sig.sidebar_sig.clear();
    last_sig.studio_sig.clear();
}

/// Preserves non-zero Streaming Economy income/expense/stall metrics across frames `430..=465`
/// even after fixed-step `SimWorld::step` ticks.
pub fn maintain_stage_13_streaming_hud_metrics(
    mod_res: &ModResource,
    sim_res: &mut SimWorldResource,
) {
    if let Some(ec) = sim_res.0.players.get_mut(&1) {
        ec.credits = ec.credits.max(2_450);
        ec.resources.insert("credits".to_string(), ec.credits);
        ec.resources.insert("aether".to_string(), 480);
        ec.resources.insert("relic_flux".to_string(), 120);
        ec.fractional_resources
            .insert("credits".to_string(), (ec.credits as i64) << 32);
        ec.fractional_resources
            .insert("aether".to_string(), 480_i64 << 32);
        ec.fractional_resources
            .insert("relic_flux".to_string(), 120_i64 << 32);
        ec.income_per_sec.insert("credits".to_string(), 68.5);
        ec.income_per_sec.insert("aether".to_string(), 24.0);
        ec.expense_per_sec.insert("credits".to_string(), 54.0);
        ec.expense_per_sec.insert("aether".to_string(), 18.0);
        ec.stall_efficiency = 0.94;
        if ec.current_era.is_none() {
            ec.current_era = mod_res
                .0
                .manifest
                .rules
                .eras
                .iter()
                .min_by_key(|e| e.order)
                .map(|e| e.id.clone());
        }
    }
}

/// Configures Stage `14` (`14_creator_workbench_node_graphs_and_vfx_studio.png`):
/// Opens Creator Workbench 2.0 (`SmokeTestEguiTarget` `1440x900`) and invokes
/// `StudioState::activate_node_graphs_and_vfx_showcase` (`Tasks 241-246 & 252`).
pub fn setup_stage_14_creator_workbench_node_graphs_and_vfx(
    mod_res: &ModResource,
    sim_res: &SimWorldResource,
    interaction: &mut ClientInteractionState,
    studio_state: &mut Option<ResMut<StudioState>>,
    keys: Option<&mut ButtonInput<KeyCode>>,
    last_sig: &mut LastSidebarSignature,
    camera_q: &mut Query<(&mut RtsCameraRig, &mut Transform), Without<PreviewStageCamera>>,
) {
    if let Some(k) = keys {
        k.release(KeyCode::ShiftLeft);
    }
    interaction.virtual_shift_preview = false;
    interaction.hud_layout_override = None;
    interaction.app_phase = AppPhase::InGame;
    interaction.menu_open = false;
    interaction.studio_open = true;
    interaction.studio_playtest_active = false;

    if let Ok((mut rig, mut tf)) = camera_q.get_single_mut() {
        rig.reset_orientation();
        rig.distance = 25.0;
        rig.target_distance = 25.0;
        if let Some(focus_ent) = sim_res
            .0
            .entities
            .values()
            .find(|e| e.player_id == 1 && matches!(e.kind, EntityKind::Unit { .. }))
        {
            rig.snap_to(Vec3::new(
                focus_ent.position[0],
                focus_ent.position[1],
                focus_ent.position[2],
            ));
        }
        *tf = rig.compute_transform();
    }

    if let Some(ref mut st) = studio_state {
        st.activate_node_graphs_and_vfx_showcase(&mod_res.0);
    }

    last_sig.studio_sig.clear();
    last_sig.sidebar_sig.clear();
}
