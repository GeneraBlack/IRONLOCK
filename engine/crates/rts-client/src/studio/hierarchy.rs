use super::dock_layout::{focus_or_open_tab, StudioTab};
use super::history::EditorCommand;
use super::{SdkInspectTarget, StudioState};
use crate::input::ClientInteractionState;
use bevy::math::Vec3;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{EntityKind, SimWorld};

pub fn show_hierarchy_outliner(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
) {
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.hierarchy_search);
        if !state.hierarchy_search.is_empty() && ui.small_button("Clear").clicked() {
            state.hierarchy_search.clear();
        }
    });

    ui.horizontal_wrapped(|ui| {
        ui.weak(format!(
            "Entities: {} | Civ: {} | Zones: {} | Ore Tiles: {}",
            world.entities.len(),
            world.civilian_structures.len(),
            loaded.entry_map.trigger_zones.len(),
            world.grid.ore_tiles.len() + world.grid.gem_tiles.len()
        ));
    });
    ui.separator();

    let query = state.hierarchy_search.trim().to_lowercase();
    let mut pending_delete_id: Option<u64> = None;

    egui::ScrollArea::vertical()
        .id_salt("studio_hierarchy_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Group 1: Players (Neutral 0 + Players 1..=8)
            for pid in 0u8..=8u8 {
                let player_entities: Vec<u64> = world
                    .entities
                    .values()
                    .filter(|e| e.player_id == pid)
                    .map(|e| e.id)
                    .collect();
                let has_slot = world.players.contains_key(&pid);
                if player_entities.is_empty() && !has_slot && pid > 2 {
                    continue;
                }
                let header = if pid == 0 {
                    format!("Neutral (Player 0) [{}]", player_entities.len())
                } else {
                    let fac = world
                        .players
                        .get(&pid)
                        .map(|p| p.faction.as_str())
                        .unwrap_or("unassigned");
                    format!("Player {pid} ({fac}) [{}]", player_entities.len())
                };

                egui::CollapsingHeader::new(header)
                    .default_open(pid <= 2)
                    .show(ui, |ui| {
                        if player_entities.is_empty() {
                            ui.weak("No spawned entities.");
                        }
                        for eid in player_entities {
                            let Some(ent) = world.entities.get(&eid) else {
                                continue;
                            };
                            let (def_id, kind_tag) = match &ent.kind {
                                EntityKind::Unit { unit_id } => (unit_id.clone(), "Unit"),
                                EntityKind::Building { building_id } => {
                                    (building_id.clone(), "Bldg")
                                }
                            };
                            if !query.is_empty()
                                && !def_id.to_lowercase().contains(&query)
                                && !eid.to_string().contains(&query)
                            {
                                continue;
                            }

                            let is_hidden = state.hidden_entity_ids.contains(&eid);
                            let is_locked = state.locked_entity_ids.contains(&eid);
                            let is_selected = interaction.selected_ids.contains(&eid);
                            let pos = ent.position;
                            let hp = ent.health;
                            let max_hp = ent.max_health;

                            ui.horizontal(|ui| {
                                let vis_lbl = if is_hidden { "🚫" } else { "👁" };
                                if ui
                                    .small_button(vis_lbl)
                                    .on_hover_text("Toggle Viewport Visibility")
                                    .clicked()
                                {
                                    if is_hidden {
                                        state.hidden_entity_ids.remove(&eid);
                                    } else {
                                        state.hidden_entity_ids.insert(eid);
                                    }
                                }
                                let lock_lbl = if is_locked { "🔒" } else { "🔓" };
                                if ui
                                    .small_button(lock_lbl)
                                    .on_hover_text("Lock / Unlock Selection & Transform")
                                    .clicked()
                                {
                                    if is_locked {
                                        state.locked_entity_ids.remove(&eid);
                                    } else {
                                        state.locked_entity_ids.insert(eid);
                                    }
                                }

                                let row_text = format!(
                                    "#{eid} [{kind_tag}] {def_id} ({:.0}/{:.0} HP)",
                                    hp, max_hp
                                );
                                let resp = ui.selectable_label(is_selected, row_text);
                                if resp.double_clicked() {
                                    state.camera_jump_request =
                                        Some(Vec3::new(pos[0], 0.0, pos[2]));
                                    state.status_banner = format!(
                                        "Camera snapped to #{eid} '{def_id}' at ({:.1}, {:.1}).",
                                        pos[0], pos[2]
                                    );
                                } else if resp.clicked() && !is_locked {
                                    interaction.selected_ids.clear();
                                    interaction.selected_ids.insert(eid);
                                    state.inspect_target = match &ent.kind {
                                        EntityKind::Unit { unit_id } => {
                                            SdkInspectTarget::Unit(unit_id.clone())
                                        }
                                        EntityKind::Building { building_id } => {
                                            SdkInspectTarget::Building(building_id.clone())
                                        }
                                    };
                                    focus_or_open_tab(
                                        &mut state.dock_state,
                                        StudioTab::PropertyInspector,
                                    );
                                }

                                if !is_locked
                                    && ui
                                        .small_button("🗑")
                                        .on_hover_text("Delete Entity (Ctrl+Z Undoable)")
                                        .clicked()
                                {
                                    pending_delete_id = Some(eid);
                                }
                            });
                        }
                    });
            }

            // Group 2: Civilian Structures (Garrisonable / Cover)
            egui::CollapsingHeader::new(format!(
                "CivilianStructures [{}]",
                world.civilian_structures.len()
            ))
            .default_open(false)
            .show(ui, |ui| {
                for civ in &world.civilian_structures {
                    let row = format!(
                        "Civ #{} @ [{}, {}] ({}x{}, {:.0}/{:.0} HP, cover {:.0}%)",
                        civ.id,
                        civ.tile_pos[0],
                        civ.tile_pos[1],
                        civ.footprint[0],
                        civ.footprint[1],
                        civ.health,
                        civ.max_health,
                        civ.cover_bonus * 100.0
                    );
                    if !query.is_empty() && !row.to_lowercase().contains(&query) {
                        continue;
                    }
                    let is_sel = state.selected_civ_ids.contains(&civ.id);
                    let resp = ui.selectable_label(is_sel, row);
                    if resp.double_clicked() {
                        state.camera_jump_request =
                            Some(Vec3::new(civ.position[0], 0.0, civ.position[2]));
                    } else if resp.clicked() {
                        interaction.selected_ids.clear();
                        state.selected_prop_ids.clear();
                        state.selected_spawn_players.clear();
                        state.selected_civ_ids.clear();
                        state.selected_civ_ids.insert(civ.id);
                        focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                    }
                }
            });

            // Group 2b: Props & Player Spawns
            egui::CollapsingHeader::new(format!(
                "Props [{}] & PlayerSpawns [{}]",
                world.props.len(),
                loaded.entry_map.spawns.len()
            ))
            .default_open(false)
            .show(ui, |ui| {
                let ts = world.grid.tile_size.max(1.0);
                for sp in &loaded.entry_map.spawns {
                    let row = format!(
                        "Spawn P{} ({}) @ [{}, {}]",
                        sp.player_id, sp.faction, sp.base_tile[0], sp.base_tile[1]
                    );
                    let is_sel = state.selected_spawn_players.contains(&sp.player_id);
                    let resp = ui.selectable_label(is_sel, row);
                    if resp.double_clicked() {
                        state.camera_jump_request = Some(Vec3::new(
                            sp.base_tile[0] as f32 * ts,
                            0.0,
                            sp.base_tile[1] as f32 * ts,
                        ));
                    } else if resp.clicked() {
                        interaction.selected_ids.clear();
                        state.selected_prop_ids.clear();
                        state.selected_civ_ids.clear();
                        state.selected_spawn_players.clear();
                        state.selected_spawn_players.insert(sp.player_id);
                        focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                    }
                }
                for prop in &world.props {
                    let row = format!(
                        "Prop #{} ({:?}) @ [{}, {}]",
                        prop.id, prop.kind, prop.tile[0], prop.tile[1]
                    );
                    if !query.is_empty() && !row.to_lowercase().contains(&query) {
                        continue;
                    }
                    let is_sel = state.selected_prop_ids.contains(&prop.id);
                    let resp = ui.selectable_label(is_sel, row);
                    if resp.double_clicked() {
                        state.camera_jump_request =
                            Some(Vec3::new(prop.position[0], 0.0, prop.position[2]));
                    } else if resp.clicked() {
                        interaction.selected_ids.clear();
                        state.selected_civ_ids.clear();
                        state.selected_spawn_players.clear();
                        state.selected_prop_ids.clear();
                        state.selected_prop_ids.insert(prop.id);
                        focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                    }
                }
            });

            // Group 3: TriggerZones
            let zone_count = loaded.entry_map.trigger_zones.len();
            egui::CollapsingHeader::new(format!("TriggerZones [{zone_count}]"))
                .default_open(false)
                .show(ui, |ui| {
                    let tile_size = loaded.entry_map.tile_size;
                    for (idx, zone) in loaded.entry_map.trigger_zones.iter().enumerate() {
                        if !query.is_empty() && !zone.id.to_lowercase().contains(&query) {
                            continue;
                        }
                        let row = format!(
                            "Zone '{}' [{},{}]..[{},{}] (actions: {})",
                            zone.id,
                            zone.min[0],
                            zone.min[1],
                            zone.max[0],
                            zone.max[1],
                            zone.actions.len()
                        );
                        let is_sel = state.selected_zone_idx == idx
                            || state.selected_zone_indices.contains(&idx);
                        let resp = ui.selectable_label(is_sel, row);
                        if resp.double_clicked() {
                            let cx = (zone.min[0] + zone.max[0]) as f32 * 0.5 * tile_size;
                            let cz = (zone.min[1] + zone.max[1]) as f32 * 0.5 * tile_size;
                            state.camera_jump_request = Some(Vec3::new(cx, 0.0, cz));
                        } else if resp.clicked() {
                            state.selected_zone_idx = idx;
                            state.selected_zone_indices.clear();
                            state.selected_zone_indices.insert(idx);
                            focus_or_open_tab(&mut state.dock_state, StudioTab::TriggerStudio);
                        }
                    }
                });

            // Group 4: ResourceNodes
            let custom_node_count: usize = world.resource_nodes.values().map(|m| m.len()).sum();
            egui::CollapsingHeader::new(format!(
                "ResourceNodes [Ore: {}, Gems: {}, Custom: {}]",
                world.grid.ore_tiles.len(),
                world.grid.gem_tiles.len(),
                custom_node_count
            ))
            .default_open(false)
            .show(ui, |ui| {
                let ts = world.grid.tile_size;
                for (idx, &seed_tile) in world.grid.ore_seed_tiles.iter().enumerate() {
                    let resp = ui.selectable_label(
                        false,
                        format!(
                            "Ore Vein #{} @ tile [{}, {}]",
                            idx + 1,
                            seed_tile[0],
                            seed_tile[1]
                        ),
                    );
                    if resp.clicked() || resp.double_clicked() {
                        state.camera_jump_request = Some(Vec3::new(
                            seed_tile[0] as f32 * ts,
                            0.0,
                            seed_tile[1] as f32 * ts,
                        ));
                    }
                }
                for (res_id, tiles) in &world.resource_nodes {
                    if let Some((&first_tile, &amt)) = tiles.iter().next() {
                        let resp = ui.selectable_label(
                            false,
                            format!(
                                "Resource '{}' ({} tiles, sample {} @ [{}, {}])",
                                res_id,
                                tiles.len(),
                                amt,
                                first_tile[0],
                                first_tile[1]
                            ),
                        );
                        if resp.clicked() || resp.double_clicked() {
                            state.camera_jump_request = Some(Vec3::new(
                                first_tile[0] as f32 * ts,
                                0.0,
                                first_tile[1] as f32 * ts,
                            ));
                        }
                    }
                }
            });
        });

    if let Some(eid) = pending_delete_id {
        if let Some(removed) = world.entities.remove(&eid) {
            interaction.selected_ids.remove(&eid);
            state.history.push_executed(EditorCommand::DeleteEntity {
                entity: Box::new(removed),
            });
            state.status_banner = format!("Deleted entity #{eid} (Undoable via Ctrl+Z).");
        }
    }
}
