mod ai_and_campaign;
mod map_and_audio;

pub use map_and_audio::show_active_map_objectives_and_cutscenes_editor;

use super::history::EditorCommand;
use super::{SdkInspectTarget, StudioState};
use ai_and_campaign::{
    show_ai_profile_inspector, show_campaign_and_mission_inspector, show_resource_inspector,
};
use bevy_egui::egui;
use map_and_audio::show_audio_catalog_inspector;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CommanderPowerNodeDef, SubFactionDef, UnitRole};
use rts_simulation::world::SimWorld;

pub(super) fn edit_optional_string(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash,
    opt: &mut Option<String>,
    hint: &str,
) -> bool {
    let mut buf = opt.clone().unwrap_or_default();
    let resp = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .id_salt(salt)
            .desired_width(155.0)
            .hint_text(hint),
    );
    if resp.changed() {
        let trimmed = buf.trim().to_string();
        *opt = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        true
    } else {
        false
    }
}

pub(super) fn edit_csv_strings(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash,
    list: &mut Vec<String>,
    hint: &str,
) -> bool {
    let mut joined = list.join(", ");
    let resp = ui.add(
        egui::TextEdit::singleline(&mut joined)
            .id_salt(salt)
            .desired_width(175.0)
            .hint_text(hint),
    );
    if resp.changed() {
        *list = joined
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        true
    } else {
        false
    }
}

/// Renders full Property Inspector GUI sections (with Undo/Redo & Disk Save) for extended Phase 21-24 schemas:
/// `UpgradeDef`, `CommanderPowerTreeDef`, `FactionDef`, `ResourceTypeDef`, `AiProfileDef`, `CampaignDef`, and `AudioCatalog`.
pub fn show_extended_schema_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) -> bool {
    match state.inspect_target.clone() {
        SdkInspectTarget::Upgrade(uid) => {
            show_upgrade_inspector(ui, state, loaded, &uid);
            true
        }
        SdkInspectTarget::CommanderTree(tid) => {
            show_commander_tree_inspector(ui, state, loaded, &tid);
            true
        }
        SdkInspectTarget::Faction(fid) => {
            show_faction_inspector(ui, state, loaded, &fid);
            true
        }
        SdkInspectTarget::Resource(rid) => {
            show_resource_inspector(ui, state, loaded, &rid);
            true
        }
        SdkInspectTarget::AiProfile(aid) => {
            show_ai_profile_inspector(ui, state, loaded, &aid);
            true
        }
        SdkInspectTarget::Campaign(cid) => {
            show_campaign_and_mission_inspector(ui, state, loaded, world, &cid);
            true
        }
        SdkInspectTarget::Audio => {
            show_audio_catalog_inspector(ui, state, loaded);
            true
        }
        _ => false,
    }
}

fn show_upgrade_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    uid: &str,
) {
    let Some(before) = loaded.upgrades.get(uid).cloned() else {
        ui.weak(format!("UpgradeDef '{uid}' not found in active mod."));
        return;
    };
    let mut draft = before.clone();

    ui.horizontal(|ui| {
        ui.strong(format!("UpgradeDef: {}", draft.id));
        if state.modified_upgrades.contains(uid) {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save to Disk").clicked() {
            match loaded.upsert_upgrade_def(draft.clone()) {
                Ok(path) => {
                    state.modified_upgrades.remove(uid);
                    state.status_banner = format!("Saved Upgrade '{}' -> {}", uid, path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save Upgrade error: {err}");
                }
            }
        }
    });
    ui.separator();

    egui::Grid::new(("ext_upg_grid", uid))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Display Name:");
            ui.text_edit_singleline(&mut draft.name);
            ui.end_row();

            ui.label("Faction:");
            egui::ComboBox::from_id_salt(("upg_fac", uid))
                .selected_text(&draft.faction)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.faction, "any".to_string(), "any");
                    ui.selectable_value(&mut draft.faction, "shared".to_string(), "shared");
                    for fid in loaded.factions.keys() {
                        ui.selectable_value(&mut draft.faction, fid.clone(), fid);
                    }
                });
            ui.end_row();

            ui.label("Cost / Research (s):");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut draft.cost)
                        .range(0..=50_000)
                        .speed(25),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.research_time_secs)
                        .range(0.5..=300.0)
                        .speed(0.5),
                );
            });
            ui.end_row();

            ui.label("Target Role:");
            let role_txt = draft
                .target_role
                .map(|r| format!("{r:?}"))
                .unwrap_or_else(|| "All Roles".to_string());
            egui::ComboBox::from_id_salt(("upg_role", uid))
                .selected_text(role_txt)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.target_role, None, "All Roles");
                    for r in [
                        UnitRole::Infantry,
                        UnitRole::Vehicle,
                        UnitRole::Harvester,
                        UnitRole::Engineer,
                    ] {
                        ui.selectable_value(&mut draft.target_role, Some(r), format!("{r:?}"));
                    }
                });
            ui.end_row();

            ui.label("Damage / Armor Mult:");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut draft.damage_bonus_mult)
                        .range(0.1..=5.0)
                        .speed(0.05)
                        .prefix("Dmg x"),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.armor_bonus_mult)
                        .range(0.1..=5.0)
                        .speed(0.05)
                        .prefix("Arm x"),
                );
            });
            ui.end_row();

            ui.label("Speed Mult / +Range:");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut draft.speed_bonus_mult)
                        .range(0.1..=5.0)
                        .speed(0.05)
                        .prefix("Spd x"),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.range_bonus)
                        .range(-10.0..=30.0)
                        .speed(0.25)
                        .prefix("+Rng "),
                );
            });
            ui.end_row();

            ui.label("Cash Bounty Ratio:");
            ui.add(
                egui::DragValue::new(&mut draft.cash_bounty_ratio)
                    .range(0.0..=2.0)
                    .speed(0.02),
            );
            ui.end_row();

            ui.label("Requires Promo Point:");
            ui.checkbox(
                &mut draft.requires_promotion_point,
                "Commander Promotion Unlock",
            );
            ui.end_row();

            ui.label("Prerequisites (CSV):");
            edit_csv_strings(
                ui,
                ("upg_prereq", uid),
                &mut draft.prerequisites,
                "building_or_upgrade_id",
            );
            ui.end_row();
        });

    if draft != before {
        loaded.upgrades.insert(uid.to_string(), draft.clone());
        state.modified_upgrades.insert(uid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditUpgradeDef {
            id: uid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }
}

fn show_commander_tree_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    tid: &str,
) {
    let Some(before) = loaded.commander_trees.get(tid).cloned() else {
        ui.weak(format!("CommanderPowerTreeDef '{tid}' not found."));
        return;
    };
    let mut draft = before.clone();

    ui.horizontal(|ui| {
        ui.strong(format!("CommanderPowerTreeDef: {}", draft.id));
        if state.modified_commander_trees.contains(tid) {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save to Disk").clicked() {
            match loaded.upsert_commander_tree_def(draft.clone()) {
                Ok(path) => {
                    state.modified_commander_trees.remove(tid);
                    state.status_banner =
                        format!("Saved Commander Tree '{}' -> {}", tid, path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save Commander Tree error: {err}");
                }
            }
        }
    });
    ui.separator();

    egui::Grid::new(("cmd_tree_meta", tid))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Display Name:");
            ui.text_edit_singleline(&mut draft.display_name);
            ui.end_row();

            ui.label("Faction:");
            egui::ComboBox::from_id_salt(("cmd_tree_fac", tid))
                .selected_text(&draft.faction)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.faction, "any".to_string(), "any");
                    for fid in loaded.factions.keys() {
                        ui.selectable_value(&mut draft.faction, fid.clone(), fid);
                    }
                });
            ui.end_row();
        });

    ui.separator();
    ui.horizontal(|ui| {
        ui.strong(format!("Nodes ({})", draft.nodes.len()));
        if ui.small_button("+ Add Node").clicked() {
            let next_idx = draft.nodes.len() + 1;
            draft.nodes.push(CommanderPowerNodeDef {
                id: format!("{}_node_{next_idx}", draft.id),
                display_name: format!("General Unlock {next_idx}"),
                icon: None,
                required_rank: 1,
                cost_points: 1,
                prerequisites: Vec::new(),
                grants_support_power: None,
                grants_upgrade: None,
                unlocks_def_id: None,
                description: String::new(),
            });
        }
    });

    let mut remove_node_idx: Option<usize> = None;
    let upgrade_ids: Vec<String> = loaded.upgrades.keys().cloned().collect();
    for (idx, node) in draft.nodes.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("#{idx}"));
                ui.add(
                    egui::TextEdit::singleline(&mut node.id)
                        .id_salt(("cnode_id", tid, idx))
                        .desired_width(120.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut node.display_name)
                        .id_salt(("cnode_nm", tid, idx))
                        .desired_width(135.0),
                );
                if ui.small_button("✖").clicked() {
                    remove_node_idx = Some(idx);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Rank:");
                ui.add(egui::DragValue::new(&mut node.required_rank).range(0..=10));
                ui.label("Pts:");
                ui.add(egui::DragValue::new(&mut node.cost_points).range(1..=10));
                ui.label("Icon:");
                edit_optional_string(ui, ("cnode_ico", tid, idx), &mut node.icon, "assets/ui/...");
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Grants Power:");
                edit_optional_string(
                    ui,
                    ("cnode_pwr", tid, idx),
                    &mut node.grants_support_power,
                    "support_power_id",
                );
                ui.label("Grants Upgrade:");
                let cur_upg = node
                    .grants_upgrade
                    .clone()
                    .unwrap_or_else(|| "(None)".to_string());
                egui::ComboBox::from_id_salt(("cnode_upg", tid, idx))
                    .selected_text(cur_upg)
                    .width(125.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut node.grants_upgrade, None, "(None)");
                        for up_id in &upgrade_ids {
                            ui.selectable_value(
                                &mut node.grants_upgrade,
                                Some(up_id.clone()),
                                up_id,
                            );
                        }
                    });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Unlocks Unit/Bldg:");
                edit_optional_string(
                    ui,
                    ("cnode_unl", tid, idx),
                    &mut node.unlocks_def_id,
                    "unit_or_building_id",
                );
                ui.label("Prereq Nodes:");
                edit_csv_strings(
                    ui,
                    ("cnode_pre", tid, idx),
                    &mut node.prerequisites,
                    "node_a, node_b",
                );
            });
            ui.add(
                egui::TextEdit::singleline(&mut node.description)
                    .id_salt(("cnode_desc", tid, idx))
                    .desired_width(f32::INFINITY)
                    .hint_text("Tooltip description..."),
            );
        });
    }
    if let Some(rem) = remove_node_idx {
        draft.nodes.remove(rem);
    }

    if draft != before {
        loaded
            .commander_trees
            .insert(tid.to_string(), draft.clone());
        state.modified_commander_trees.insert(tid.to_string());
        state.validation_dirty = true;
        state
            .history
            .push_executed(EditorCommand::EditCommanderTreeDef {
                id: tid.to_string(),
                before: Box::new(before),
                after: Box::new(draft),
            });
    }
}

fn show_faction_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    fid: &str,
) {
    let Some(before) = loaded.factions.get(fid).cloned() else {
        ui.weak(format!("FactionDef '{fid}' not found."));
        return;
    };
    let mut draft = before.clone();

    ui.horizontal(|ui| {
        ui.strong(format!("FactionDef: {}", draft.id));
        if state.modified_factions.contains(fid) {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save to Disk").clicked() {
            match loaded.upsert_faction_def(draft.clone()) {
                Ok(path) => {
                    state.modified_factions.remove(fid);
                    state.status_banner = format!("Saved Faction '{}' -> {}", fid, path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save Faction error: {err}");
                }
            }
        }
    });
    ui.separator();

    egui::Grid::new(("fac_meta", fid))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Display Name:");
            ui.text_edit_singleline(&mut draft.name);
            ui.end_row();

            ui.label("Team Color & Credits:");
            ui.horizontal(|ui| {
                ui.color_edit_button_rgb(&mut draft.color);
                ui.add(
                    egui::DragValue::new(&mut draft.starting_credits)
                        .range(0..=1_000_000)
                        .speed(250)
                        .prefix("$"),
                );
            });
            ui.end_row();

            ui.label("Description:");
            ui.text_edit_singleline(&mut draft.description);
            ui.end_row();

            ui.label("Loading Screen:");
            edit_optional_string(
                ui,
                ("fac_load", fid),
                &mut draft.loading_screen,
                "assets/ui/loading.png",
            );
            ui.end_row();

            ui.label("Campaign Screens:");
            edit_csv_strings(
                ui,
                ("fac_camp_screens", fid),
                &mut draft.campaign_loading_screens,
                "screen1.png, screen2.png",
            );
            ui.end_row();
        });

    egui::CollapsingHeader::new(format!("Sub-Factions ({})", draft.sub_factions.len()))
        .default_open(true)
        .show(ui, |ui| {
            if ui.small_button("+ Add Sub-Faction").clicked() {
                let n = draft.sub_factions.len() + 1;
                draft.sub_factions.push(SubFactionDef {
                    id: format!("{}_sub_{n}", draft.id),
                    name: format!("Sub-Faction {n}"),
                    specialty: "Armored Division".to_string(),
                    loading_screen: None,
                });
            }
            let mut rem_sub = None;
            for (idx, sf) in draft.sub_factions.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut sf.id)
                            .id_salt(("sf_id", fid, idx))
                            .desired_width(95.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut sf.name)
                            .id_salt(("sf_nm", fid, idx))
                            .desired_width(110.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut sf.specialty)
                            .id_salt(("sf_sp", fid, idx))
                            .desired_width(130.0)
                            .hint_text("Specialty"),
                    );
                    edit_optional_string(
                        ui,
                        ("sf_ld", fid, idx),
                        &mut sf.loading_screen,
                        "loading.png",
                    );
                    if ui.small_button("✖").clicked() {
                        rem_sub = Some(idx);
                    }
                });
            }
            if let Some(i) = rem_sub {
                draft.sub_factions.remove(i);
            }
        });

    egui::CollapsingHeader::new("HUD Skin Overrides (`FactionHudSkinDef`)")
        .default_open(false)
        .show(ui, |ui| {
            let skin = &mut draft.hud_skin;
            egui::Grid::new(("fac_skin_grid", fid))
                .num_columns(2)
                .spacing([8.0, 3.0])
                .show(ui, |ui| {
                    let mut row = |lbl: &str, key: &str, slot: &mut Option<String>| {
                        ui.label(lbl);
                        edit_optional_string(ui, ("fskin", fid, key), slot, "assets/ui/...");
                        ui.end_row();
                    };
                    row("Top Panel:", "top", &mut skin.top_panel);
                    row("Credits Panel:", "cred", &mut skin.credits_panel);
                    row("Radar Frame:", "radar", &mut skin.radar_frame);
                    row("Side Header:", "shead", &mut skin.side_header);
                    row("Cameo Row:", "crow", &mut skin.cameo_row_frame);
                    row("Cameo Footer:", "cfoot", &mut skin.cameo_footer_frame);
                    row("Addon Panel:", "addon", &mut skin.addon_panel);
                    row("Power Bar Icon:", "pwr", &mut skin.power_bar_icon);
                    row("Repair Button:", "rep", &mut skin.repair_button);
                    row("Sell Button:", "sell", &mut skin.sell_button);
                    row("Options Button:", "opt", &mut skin.options_button);
                    row("Diplomacy Btn:", "dip", &mut skin.diplomacy_button);
                    row("Dialog Bg:", "dlg", &mut skin.dialog_background);
                    ui.label("Tab Icons (CSV):");
                    edit_csv_strings(
                        ui,
                        ("fskin_tabs", fid),
                        &mut skin.tab_icons,
                        "tab0.png, tab1.png",
                    );
                    ui.end_row();
                });
        });

    if draft != before {
        loaded.factions.insert(fid.to_string(), draft.clone());
        state.modified_factions.insert(fid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditFactionDef {
            id: fid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }
}
