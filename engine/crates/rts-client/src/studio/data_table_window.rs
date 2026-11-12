use super::dock_layout::{focus_or_open_tab, StudioTab};
use super::history::EditorCommand;
use super::{SdkInspectTarget, StudioState};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataTableCategory {
    #[default]
    Units,
    Buildings,
    Weapons,
    Upgrades,
    Factions,
}

pub fn show_data_table_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.table_category, DataTableCategory::Units, "Units");
        ui.selectable_value(
            &mut state.table_category,
            DataTableCategory::Buildings,
            "Buildings",
        );
        ui.selectable_value(
            &mut state.table_category,
            DataTableCategory::Weapons,
            "Weapons",
        );
        ui.selectable_value(
            &mut state.table_category,
            DataTableCategory::Upgrades,
            "Upgrades",
        );
        ui.selectable_value(
            &mut state.table_category,
            DataTableCategory::Factions,
            "Factions",
        );
        ui.separator();
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.table_filter);
        let dirty_total = state.modified_units.len()
            + state.modified_buildings.len()
            + state.modified_weapons.len()
            + state.modified_upgrades.len()
            + state.modified_commander_trees.len()
            + state.modified_factions.len()
            + state.modified_resources.len()
            + state.modified_ai_profiles.len()
            + state.modified_campaigns.len()
            + usize::from(state.audio_modified)
            + state.modified_locales.len();
        if ui
            .button(format!("💾 Save All Modified ({dirty_total})"))
            .clicked()
        {
            save_all_modified_defs(state, loaded, world);
        }
    });

    ui.separator();

    let query = state.table_filter.trim().to_lowercase();
    egui::ScrollArea::both()
        .id_salt("studio_data_table_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| match state.table_category {
            DataTableCategory::Units => render_units_table(ui, state, loaded, world, &query),
            DataTableCategory::Buildings => {
                render_buildings_table(ui, state, loaded, world, &query)
            }
            DataTableCategory::Weapons => render_weapons_table(ui, state, loaded, world, &query),
            DataTableCategory::Upgrades => render_upgrades_table(ui, state, loaded, &query),
            DataTableCategory::Factions => render_factions_table(ui, state, loaded, &query),
        });
}

pub fn save_all_modified_defs(
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let mut saved_count = 0usize;
    let uids: Vec<String> = state.modified_units.iter().cloned().collect();
    for uid in uids {
        if let Some(u) = loaded.units.get(&uid).cloned() {
            if loaded.save_unit_def_toml(u).is_ok() {
                state.modified_units.remove(&uid);
                saved_count += 1;
            }
        }
    }
    let bids: Vec<String> = state.modified_buildings.iter().cloned().collect();
    for bid in bids {
        if let Some(b) = loaded.buildings.get(&bid).cloned() {
            if loaded.save_building_def_toml(b).is_ok() {
                state.modified_buildings.remove(&bid);
                saved_count += 1;
            }
        }
    }
    let wids: Vec<String> = state.modified_weapons.iter().cloned().collect();
    for wid in wids {
        if let Some(w) = loaded.weapons.get(&wid).cloned() {
            if loaded.save_weapon_def_toml(w).is_ok() {
                state.modified_weapons.remove(&wid);
                saved_count += 1;
            }
        }
    }
    let upids: Vec<String> = state.modified_upgrades.iter().cloned().collect();
    for upid in upids {
        if let Some(up) = loaded.upgrades.get(&upid).cloned() {
            if loaded.upsert_upgrade_def(up).is_ok() {
                state.modified_upgrades.remove(&upid);
                saved_count += 1;
            }
        }
    }
    let ctids: Vec<String> = state.modified_commander_trees.iter().cloned().collect();
    for ctid in ctids {
        if let Some(ct) = loaded.commander_trees.get(&ctid).cloned() {
            if loaded.upsert_commander_tree_def(ct).is_ok() {
                state.modified_commander_trees.remove(&ctid);
                saved_count += 1;
            }
        }
    }
    let fids: Vec<String> = state.modified_factions.iter().cloned().collect();
    for fid in fids {
        if let Some(f) = loaded.factions.get(&fid).cloned() {
            if loaded.upsert_faction_def(f).is_ok() {
                state.modified_factions.remove(&fid);
                saved_count += 1;
            }
        }
    }
    let rids: Vec<String> = state.modified_resources.iter().cloned().collect();
    for rid in rids {
        if let Some(r) = loaded.resources.get(&rid).cloned() {
            if loaded.upsert_resource_type_def(r).is_ok() {
                state.modified_resources.remove(&rid);
                saved_count += 1;
            }
        }
    }
    let aids: Vec<String> = state.modified_ai_profiles.iter().cloned().collect();
    for aid in aids {
        if let Some(ai) = loaded.ai_profiles.get(&aid).cloned() {
            if loaded.upsert_ai_profile_def(ai).is_ok() {
                state.modified_ai_profiles.remove(&aid);
                saved_count += 1;
            }
        }
    }
    let cids: Vec<String> = state.modified_campaigns.iter().cloned().collect();
    for cid in cids {
        if let Some(c) = loaded.campaigns.get(&cid).cloned() {
            if loaded.upsert_campaign_def(c).is_ok() {
                state.modified_campaigns.remove(&cid);
                saved_count += 1;
            }
        }
    }
    if state.audio_modified && loaded.save_audio_catalog(loaded.audio.clone()).is_ok() {
        state.audio_modified = false;
        saved_count += 1;
    }
    let langs: Vec<String> = state.modified_locales.iter().cloned().collect();
    for lang in langs {
        if let Some(loc) = loaded.locales.get(&lang).cloned() {
            if loaded.save_locale_catalog(loc).is_ok() {
                state.modified_locales.remove(&lang);
                saved_count += 1;
            }
        }
    }
    let _ = world.apply_mod_hot_reload(loaded);
    state.validation_dirty = true;
    state.status_banner = format!("Saved {saved_count} modified SDK definition(s) to TOML.");
}

fn sort_header_btn(ui: &mut egui::Ui, state: &mut StudioState, col: usize, title: &str) {
    let suffix = if state.table_sort_col == col {
        if state.table_sort_asc {
            " ▲"
        } else {
            " ▼"
        }
    } else {
        ""
    };
    if ui.button(format!("{title}{suffix}")).clicked() {
        if state.table_sort_col == col {
            state.table_sort_asc = !state.table_sort_asc;
        } else {
            state.table_sort_col = col;
            state.table_sort_asc = true;
        }
    }
}

fn render_units_table(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    query: &str,
) {
    let mut ids: Vec<String> = loaded
        .units
        .values()
        .filter(|u| {
            query.is_empty()
                || u.id.to_lowercase().contains(query)
                || u.name.to_lowercase().contains(query)
                || u.faction.to_lowercase().contains(query)
        })
        .map(|u| u.id.clone())
        .collect();

    ids.sort_by(|a, b| {
        let ua = &loaded.units[a];
        let ub = &loaded.units[b];
        let ord = match state.table_sort_col {
            1 => ua.name.cmp(&ub.name),
            2 => ua.faction.cmp(&ub.faction),
            3 => ua.cost.cmp(&ub.cost),
            4 => ua.health.total_cmp(&ub.health),
            5 => ua.speed.total_cmp(&ub.speed),
            6 => ua.build_time_secs.total_cmp(&ub.build_time_secs),
            _ => ua.id.cmp(&ub.id),
        };
        if state.table_sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    egui::Grid::new("studio_units_spreadsheet")
        .striped(true)
        .min_col_width(58.0)
        .show(ui, |ui| {
            sort_header_btn(ui, state, 0, "Unit ID");
            sort_header_btn(ui, state, 1, "Name");
            sort_header_btn(ui, state, 2, "Faction");
            sort_header_btn(ui, state, 3, "Cost ($)");
            sort_header_btn(ui, state, 4, "Health");
            sort_header_btn(ui, state, 5, "Speed");
            sort_header_btn(ui, state, 6, "Build(s)");
            ui.strong("DPS");
            ui.end_row();

            for uid in ids {
                let Some(before) = loaded.units.get(&uid).cloned() else {
                    continue;
                };
                let mut draft = before.clone();
                let dps = draft
                    .primary_weapon
                    .as_ref()
                    .and_then(|wid| loaded.weapons.get(wid))
                    .map(|w| w.damage * w.burst.max(1) as f32 / w.cooldown_secs.max(0.05))
                    .unwrap_or(0.0);

                if ui.small_button(&uid).clicked() {
                    state.inspect_target = SdkInspectTarget::Unit(uid.clone());
                    focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                }
                ui.label(&draft.name);
                ui.label(&draft.faction);
                ui.add(
                    egui::DragValue::new(&mut draft.cost)
                        .range(0..=20000)
                        .speed(10),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.health)
                        .range(10.0..=20000.0)
                        .speed(5.0),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.speed)
                        .range(0.0..=30.0)
                        .speed(0.1),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.build_time_secs)
                        .range(0.5..=240.0)
                        .speed(0.5),
                );
                ui.label(format!("{dps:.1}"));
                ui.end_row();

                if draft != before {
                    loaded.units.insert(uid.clone(), draft.clone());
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.modified_units.insert(uid.clone());
                    state.validation_dirty = true;
                    state.history.push_executed(EditorCommand::EditUnitDef {
                        id: uid,
                        before: Box::new(before),
                        after: Box::new(draft),
                    });
                }
            }
        });
}

fn render_buildings_table(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    query: &str,
) {
    let mut ids: Vec<String> = loaded
        .buildings
        .values()
        .filter(|b| {
            query.is_empty()
                || b.id.to_lowercase().contains(query)
                || b.name.to_lowercase().contains(query)
                || b.faction.to_lowercase().contains(query)
        })
        .map(|b| b.id.clone())
        .collect();

    ids.sort_by(|a, b| {
        let ba = &loaded.buildings[a];
        let bb = &loaded.buildings[b];
        let ord = match state.table_sort_col {
            1 => ba.name.cmp(&bb.name),
            2 => ba.faction.cmp(&bb.faction),
            3 => ba.cost.cmp(&bb.cost),
            4 => ba.health.total_cmp(&bb.health),
            5 => ba.power_delta.cmp(&bb.power_delta),
            6 => ba.build_time_secs.total_cmp(&bb.build_time_secs),
            _ => ba.id.cmp(&bb.id),
        };
        if state.table_sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    egui::Grid::new("studio_buildings_spreadsheet")
        .striped(true)
        .min_col_width(58.0)
        .show(ui, |ui| {
            sort_header_btn(ui, state, 0, "Building ID");
            sort_header_btn(ui, state, 1, "Name");
            sort_header_btn(ui, state, 2, "Faction");
            sort_header_btn(ui, state, 3, "Cost ($)");
            sort_header_btn(ui, state, 4, "Health");
            sort_header_btn(ui, state, 5, "Power (MW)");
            sort_header_btn(ui, state, 6, "Build(s)");
            ui.strong("Footprint");
            ui.end_row();

            for bid in ids {
                let Some(before) = loaded.buildings.get(&bid).cloned() else {
                    continue;
                };
                let mut draft = before.clone();

                if ui.small_button(&bid).clicked() {
                    state.inspect_target = SdkInspectTarget::Building(bid.clone());
                    focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                }
                ui.label(&draft.name);
                ui.label(&draft.faction);
                ui.add(
                    egui::DragValue::new(&mut draft.cost)
                        .range(0..=20000)
                        .speed(10),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.health)
                        .range(50.0..=30000.0)
                        .speed(10.0),
                );
                ui.add(egui::DragValue::new(&mut draft.power_delta).range(-800..=1200));
                ui.add(
                    egui::DragValue::new(&mut draft.build_time_secs)
                        .range(0.5..=300.0)
                        .speed(0.5),
                );
                ui.label(format!("{}x{}", draft.footprint[0], draft.footprint[1]));
                ui.end_row();

                if draft != before {
                    loaded.buildings.insert(bid.clone(), draft.clone());
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.modified_buildings.insert(bid.clone());
                    state.validation_dirty = true;
                    state.history.push_executed(EditorCommand::EditBuildingDef {
                        id: bid,
                        before: Box::new(before),
                        after: Box::new(draft),
                    });
                }
            }
        });
}

fn render_weapons_table(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    query: &str,
) {
    let mut ids: Vec<String> = loaded
        .weapons
        .values()
        .filter(|w| {
            query.is_empty()
                || w.id.to_lowercase().contains(query)
                || w.name.to_lowercase().contains(query)
        })
        .map(|w| w.id.clone())
        .collect();

    ids.sort_by(|a, b| {
        let wa = &loaded.weapons[a];
        let wb = &loaded.weapons[b];
        let ord = match state.table_sort_col {
            1 => wa.name.cmp(&wb.name),
            3 => wa.damage.total_cmp(&wb.damage),
            4 => wa.range.total_cmp(&wb.range),
            5 => wa.cooldown_secs.total_cmp(&wb.cooldown_secs),
            6 => wa.splash_radius.total_cmp(&wb.splash_radius),
            _ => wa.id.cmp(&wb.id),
        };
        if state.table_sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    egui::Grid::new("studio_weapons_spreadsheet")
        .striped(true)
        .min_col_width(58.0)
        .show(ui, |ui| {
            sort_header_btn(ui, state, 0, "Weapon ID");
            sort_header_btn(ui, state, 1, "Name");
            ui.strong("Delivery");
            sort_header_btn(ui, state, 3, "Damage");
            sort_header_btn(ui, state, 4, "Range");
            sort_header_btn(ui, state, 5, "Cooldown");
            sort_header_btn(ui, state, 6, "Splash");
            ui.strong("DPS");
            ui.end_row();

            for wid in ids {
                let Some(before) = loaded.weapons.get(&wid).cloned() else {
                    continue;
                };
                let mut draft = before.clone();
                let dps = draft.damage * draft.burst.max(1) as f32 / draft.cooldown_secs.max(0.05);

                if ui.small_button(&wid).clicked() {
                    state.inspect_target = SdkInspectTarget::Weapon(wid.clone());
                    focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                }
                ui.label(&draft.name);
                ui.label(format!("{:?}", draft.delivery));
                ui.add(
                    egui::DragValue::new(&mut draft.damage)
                        .range(0.0..=10000.0)
                        .speed(2.0),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.range)
                        .range(0.5..=60.0)
                        .speed(0.2),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.cooldown_secs)
                        .range(0.05..=30.0)
                        .speed(0.05),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.splash_radius)
                        .range(0.0..=16.0)
                        .speed(0.1),
                );
                ui.label(format!("{dps:.1}"));
                ui.end_row();

                if draft != before {
                    loaded.weapons.insert(wid.clone(), draft.clone());
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.modified_weapons.insert(wid.clone());
                    state.validation_dirty = true;
                    state.history.push_executed(EditorCommand::EditWeaponDef {
                        id: wid,
                        before: Box::new(before),
                        after: Box::new(draft),
                    });
                }
            }
        });
}

fn render_upgrades_table(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    query: &str,
) {
    let mut ids: Vec<String> = loaded
        .upgrades
        .values()
        .filter(|u| {
            query.is_empty()
                || u.id.to_lowercase().contains(query)
                || u.name.to_lowercase().contains(query)
                || u.faction.to_lowercase().contains(query)
        })
        .map(|u| u.id.clone())
        .collect();

    ids.sort_by(|a, b| {
        let ua = &loaded.upgrades[a];
        let ub = &loaded.upgrades[b];
        let ord = match state.table_sort_col {
            1 => ua.name.cmp(&ub.name),
            2 => ua.faction.cmp(&ub.faction),
            3 => ua.cost.cmp(&ub.cost),
            4 => ua.research_time_secs.total_cmp(&ub.research_time_secs),
            5 => ua.damage_bonus_mult.total_cmp(&ub.damage_bonus_mult),
            6 => ua.armor_bonus_mult.total_cmp(&ub.armor_bonus_mult),
            _ => ua.id.cmp(&ub.id),
        };
        if state.table_sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    egui::Grid::new("studio_upgrades_spreadsheet")
        .striped(true)
        .min_col_width(58.0)
        .show(ui, |ui| {
            sort_header_btn(ui, state, 0, "Upgrade ID");
            sort_header_btn(ui, state, 1, "Name");
            sort_header_btn(ui, state, 2, "Faction");
            sort_header_btn(ui, state, 3, "Cost ($)");
            sort_header_btn(ui, state, 4, "Time (s)");
            sort_header_btn(ui, state, 5, "Dmg Mult");
            sort_header_btn(ui, state, 6, "Arm Mult");
            ui.strong("Spd Mult");
            ui.end_row();

            for uid in ids {
                let Some(before) = loaded.upgrades.get(&uid).cloned() else {
                    continue;
                };
                let mut draft = before.clone();

                if ui.small_button(&uid).clicked() {
                    state.inspect_target = SdkInspectTarget::Upgrade(uid.clone());
                    focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                }
                ui.label(&draft.name);
                ui.label(&draft.faction);
                ui.add(
                    egui::DragValue::new(&mut draft.cost)
                        .range(0..=50000)
                        .speed(25),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.research_time_secs)
                        .range(0.5..=300.0)
                        .speed(0.5),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.damage_bonus_mult)
                        .range(0.1..=5.0)
                        .speed(0.05),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.armor_bonus_mult)
                        .range(0.1..=5.0)
                        .speed(0.05),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.speed_bonus_mult)
                        .range(0.1..=5.0)
                        .speed(0.05),
                );
                ui.end_row();

                if draft != before {
                    loaded.upgrades.insert(uid.clone(), draft.clone());
                    state.modified_upgrades.insert(uid.clone());
                    state.validation_dirty = true;
                    state.history.push_executed(EditorCommand::EditUpgradeDef {
                        id: uid,
                        before: Box::new(before),
                        after: Box::new(draft),
                    });
                }
            }
        });
}

fn render_factions_table(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    query: &str,
) {
    let mut ids: Vec<String> = loaded
        .factions
        .values()
        .filter(|f| {
            query.is_empty()
                || f.id.to_lowercase().contains(query)
                || f.name.to_lowercase().contains(query)
        })
        .map(|f| f.id.clone())
        .collect();

    ids.sort_by(|a, b| {
        let fa = &loaded.factions[a];
        let fb = &loaded.factions[b];
        let ord = match state.table_sort_col {
            1 => fa.name.cmp(&fb.name),
            2 => fa.starting_credits.cmp(&fb.starting_credits),
            _ => fa.id.cmp(&fb.id),
        };
        if state.table_sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    egui::Grid::new("studio_factions_spreadsheet")
        .striped(true)
        .min_col_width(64.0)
        .show(ui, |ui| {
            sort_header_btn(ui, state, 0, "Faction ID");
            sort_header_btn(ui, state, 1, "Display Name");
            sort_header_btn(ui, state, 2, "Starting $");
            ui.strong("Color");
            ui.strong("Sub-Factions");
            ui.end_row();

            for fid in ids {
                let Some(before) = loaded.factions.get(&fid).cloned() else {
                    continue;
                };
                let mut draft = before.clone();

                if ui.small_button(&fid).clicked() {
                    state.inspect_target = SdkInspectTarget::Faction(fid.clone());
                    focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
                }
                ui.text_edit_singleline(&mut draft.name);
                ui.add(
                    egui::DragValue::new(&mut draft.starting_credits)
                        .range(0..=1_000_000)
                        .speed(250),
                );
                ui.color_edit_button_rgb(&mut draft.color);
                ui.label(format!("{}", draft.sub_factions.len()));
                ui.end_row();

                if draft != before {
                    loaded.factions.insert(fid.clone(), draft.clone());
                    state.modified_factions.insert(fid.clone());
                    state.validation_dirty = true;
                    state.history.push_executed(EditorCommand::EditFactionDef {
                        id: fid,
                        before: Box::new(before),
                        after: Box::new(draft),
                    });
                }
            }
        });
}
