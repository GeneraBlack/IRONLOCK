use super::dock_layout::{focus_or_open_tab, StudioTab};
use super::history::EditorCommand;
use super::inspector_extended_schemas::show_extended_schema_inspector;
use super::{SdkInspectTarget, StudioState};
use crate::input::ClientInteractionState;
use bevy_egui::egui;
use building_and_traits::{show_building_def_inspector, show_modular_traits_editor};
use combat_and_visuals::show_shared_combat_specs_and_visuals;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ArmorType, LocomotorKind, MovementDomain, UnitDef, UnitRole};
use rts_sdk::validator::Severity;
use rts_simulation::world::SimWorld;
use weapons_and_widgets::{
    edit_csv_inline, edit_optional_ref_dropdown, edit_resource_map, edit_string_list_dropdown,
    show_selected_map_entity_section, show_weapon_def_inspector,
};

mod building_and_traits;
mod combat_and_visuals;
mod weapons_and_widgets;

pub(super) const ALL_ARMORS: [ArmorType; 5] = [
    ArmorType::None,
    ArmorType::Light,
    ArmorType::Medium,
    ArmorType::Heavy,
    ArmorType::Structure,
];

const ALL_DOMAINS: [MovementDomain; 6] = [
    MovementDomain::Ground,
    MovementDomain::Air,
    MovementDomain::Naval,
    MovementDomain::Amphibious,
    MovementDomain::Jumpjet,
    MovementDomain::Chrono,
];

const ALL_LOCOMOTORS: [LocomotorKind; 7] = [
    LocomotorKind::Infantry,
    LocomotorKind::Wheeled,
    LocomotorKind::Tracked,
    LocomotorKind::Hover,
    LocomotorKind::Naval,
    LocomotorKind::FixedWingAircraft,
    LocomotorKind::Vtol,
];

const ALL_UNIT_ROLES: [UnitRole; 4] = [
    UnitRole::Infantry,
    UnitRole::Vehicle,
    UnitRole::Harvester,
    UnitRole::Engineer,
];

pub fn show_property_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
) {
    // Quick SDK Target Picker Strip across all schemas
    ui.horizontal_wrapped(|ui| {
        ui.label("SDK:");
        egui::ComboBox::from_id_salt("inspector_unit_pick")
            .selected_text("Units")
            .width(74.0)
            .show_ui(ui, |ui| {
                let ids: Vec<String> = loaded.units.keys().cloned().collect();
                for uid in ids {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Unit(uid.clone()),
                            &uid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Unit(uid);
                    }
                }
            });
        egui::ComboBox::from_id_salt("inspector_bldg_pick")
            .selected_text("Bldgs")
            .width(74.0)
            .show_ui(ui, |ui| {
                let ids: Vec<String> = loaded.buildings.keys().cloned().collect();
                for bid in ids {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Building(bid.clone()),
                            &bid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Building(bid);
                    }
                }
            });
        egui::ComboBox::from_id_salt("inspector_wpn_pick")
            .selected_text("Wpns")
            .width(72.0)
            .show_ui(ui, |ui| {
                let ids: Vec<String> = loaded.weapons.keys().cloned().collect();
                for wid in ids {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Weapon(wid.clone()),
                            &wid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Weapon(wid);
                    }
                }
            });
        egui::ComboBox::from_id_salt("inspector_upg_pick")
            .selected_text("Upgr")
            .width(70.0)
            .show_ui(ui, |ui| {
                let ids: Vec<String> = loaded.upgrades.keys().cloned().collect();
                for uid in ids {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Upgrade(uid.clone()),
                            &uid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Upgrade(uid);
                    }
                }
                let tids: Vec<String> = loaded.commander_trees.keys().cloned().collect();
                for tid in tids {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::CommanderTree(tid.clone()),
                            format!("🎖 {tid}"),
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::CommanderTree(tid);
                    }
                }
            });
        egui::ComboBox::from_id_salt("inspector_more_pick")
            .selected_text("More...")
            .width(78.0)
            .show_ui(ui, |ui| {
                ui.strong("Factions");
                for fid in loaded.factions.keys() {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Faction(fid.clone()),
                            fid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Faction(fid.clone());
                    }
                }
                ui.separator();
                ui.strong("Resources");
                for rid in loaded.resources.keys() {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Resource(rid.clone()),
                            rid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Resource(rid.clone());
                    }
                }
                ui.separator();
                ui.strong("AI Profiles");
                for aid in loaded.ai_profiles.keys() {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::AiProfile(aid.clone()),
                            aid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::AiProfile(aid.clone());
                    }
                }
                ui.separator();
                ui.strong("Campaigns");
                for cid in loaded.campaigns.keys() {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Campaign(cid.clone()),
                            cid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Campaign(cid.clone());
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(
                        state.inspect_target == SdkInspectTarget::Audio,
                        "🔊 AudioCatalog",
                    )
                    .clicked()
                {
                    state.inspect_target = SdkInspectTarget::Audio;
                }
            });
    });

    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("studio_property_inspector_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Section 1: Selected Live Map Entity / Map Object Inspector
            if let Some(&sel_id) = interaction.selected_ids.iter().next() {
                show_selected_map_entity_section(ui, sel_id, state, world, interaction);
                ui.separator();
            }
            if let Some(&sp_pid) = state.selected_spawn_players.iter().next() {
                if let Some(sp) = loaded
                    .entry_map
                    .spawns
                    .iter_mut()
                    .find(|s| s.player_id == sp_pid)
                {
                    egui::CollapsingHeader::new(format!("Selected Player Spawn (P{sp_pid})"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Base Tile:");
                                ui.add(egui::DragValue::new(&mut sp.base_tile[0]).prefix("X: "));
                                ui.add(egui::DragValue::new(&mut sp.base_tile[1]).prefix("Z: "));
                                ui.label("Team:");
                                ui.add(egui::DragValue::new(&mut sp.team_id).range(0..=8));
                            });
                        });
                    ui.separator();
                }
            }
            if let Some(&cid) = state.selected_civ_ids.iter().next() {
                if let Some(civ) = world.civilian_structures.iter_mut().find(|c| c.id == cid) {
                    egui::CollapsingHeader::new(format!("Selected Civilian Structure #{cid}"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.label(format!(
                                "Tile [{}, {}] | Footprint {}x{}",
                                civ.tile_pos[0],
                                civ.tile_pos[1],
                                civ.footprint[0],
                                civ.footprint[1]
                            ));
                            let mhp = civ.max_health.max(1.0);
                            ui.add(egui::Slider::new(&mut civ.health, 1.0..=mhp).text("HP"));
                            ui.add(
                                egui::Slider::new(&mut civ.cover_bonus, 0.0..=0.85).text("Cover"),
                            );
                        });
                    ui.separator();
                }
            }
            if let Some(&pid) = state.selected_prop_ids.iter().next() {
                if let Some(prop) = world.props.iter().find(|p| p.id == pid) {
                    egui::CollapsingHeader::new(format!("Selected Battlefield Prop #{pid}"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.label(format!(
                                "Kind: {:?} | Tile [{}, {}] | World ({:.1}, {:.1}, {:.1})",
                                prop.kind,
                                prop.tile[0],
                                prop.tile[1],
                                prop.position[0],
                                prop.position[1],
                                prop.position[2]
                            ));
                        });
                    ui.separator();
                }
            }

            // Section 2: Selected SDK Definition Inspector
            if show_extended_schema_inspector(ui, state, loaded, world) {
                return;
            }
            match state.inspect_target.clone() {
                SdkInspectTarget::None => {
                    ui.weak(
                        "Select any Unit, Building, Weapon, Upgrade, Faction, Resource, AI Profile, Campaign, or AudioCatalog in Asset Browser (Tab 1) or dropdowns above.",
                    );
                }
                SdkInspectTarget::Unit(uid) => {
                    show_unit_def_inspector(ui, &uid, state, loaded, world);
                }
                SdkInspectTarget::Building(bid) => {
                    show_building_def_inspector(ui, &bid, state, loaded, world);
                }
                SdkInspectTarget::Weapon(wid) => {
                    show_weapon_def_inspector(ui, &wid, state, loaded, world);
                }
                _ => {}
            }
        });
}

pub(super) fn show_inline_validation_badges(ui: &mut egui::Ui, state: &StudioState, def_id: &str) {
    let Some(report) = state.cached_validation_report.as_ref() else {
        return;
    };
    let matching: Vec<_> = report
        .issues
        .iter()
        .filter(|iss| iss.message.contains(def_id))
        .collect();
    if matching.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(90, 220, 130),
            "✔ Schema Valid (0 issues referencing this ID)",
        );
    } else {
        for iss in matching {
            let col = match iss.severity {
                Severity::Error => egui::Color32::from_rgb(245, 95, 95),
                Severity::Warning => egui::Color32::from_rgb(245, 195, 85),
            };
            ui.colored_label(
                col,
                format!("[{:?} {}] {}", iss.severity, iss.code, iss.message),
            );
        }
    }
}

fn show_unit_def_inspector(
    ui: &mut egui::Ui,
    uid: &str,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let Some(before) = loaded.units.get(uid).cloned() else {
        ui.weak(format!("Unit '{uid}' not found in active mod."));
        return;
    };
    let mut draft: UnitDef = before.clone();
    let faction_ids: Vec<String> = loaded.factions.keys().cloned().collect();
    let weapon_ids: Vec<String> = loaded.weapons.keys().cloned().collect();
    let building_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
    let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
    let resource_ids: Vec<String> = loaded.resources.keys().cloned().collect();
    let vfx_ids: Vec<String> = loaded.vfx_effects.keys().cloned().collect();

    ui.horizontal_wrapped(|ui| {
        ui.heading(format!("UnitDef: {uid}"));
        if ui
            .button("💾 Save to .toml & Hot-Reload")
            .on_hover_text(
                "Writes roundtrip TOML via LoadedMod::save_unit_def_toml and hot-reloads SimWorld",
            )
            .clicked()
        {
            match loaded.save_unit_def_toml(draft.clone()) {
                Ok(path) => {
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.modified_units.remove(uid);
                    state.validation_dirty = true;
                    state.status_banner = format!("Saved Unit '{uid}' -> {}", path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save error for '{uid}': {err}");
                }
            }
        }
        if ui.small_button("🎨 PBR Material Studio").clicked() {
            state.material_studio_ui.override_enabled = true;
            focus_or_open_tab(&mut state.dock_state, StudioTab::AssetPreviewer);
        }
        if ui.small_button("✨ VFX Studio").clicked() {
            focus_or_open_tab(&mut state.dock_state, StudioTab::VfxStudio);
        }
    });
    show_inline_validation_badges(ui, state, uid);

    egui::CollapsingHeader::new("Core Stats & Economy")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Display Name:");
                ui.text_edit_singleline(&mut draft.name);
            });
            ui.horizontal(|ui| {
                ui.label("Faction:");
                egui::ComboBox::from_id_salt("unit_faction_cb")
                    .selected_text(&draft.faction)
                    .show_ui(ui, |ui| {
                        for fid in &faction_ids {
                            ui.selectable_value(&mut draft.faction, fid.clone(), fid);
                        }
                    });
                ui.label("Role:");
                egui::ComboBox::from_id_salt("unit_role_cb")
                    .selected_text(format!("{:?}", draft.role))
                    .show_ui(ui, |ui| {
                        for r in ALL_UNIT_ROLES {
                            ui.selectable_value(&mut draft.role, r, format!("{r:?}"));
                        }
                    });
            });
            ui.add(egui::Slider::new(&mut draft.cost, 0..=10000).text("Credit Cost ($)"));
            ui.add(
                egui::Slider::new(&mut draft.build_time_secs, 0.5..=180.0).text("Build Time (s)"),
            );
            ui.add(egui::Slider::new(&mut draft.health, 10.0..=12000.0).text("Max Health"));
            ui.horizontal(|ui| {
                ui.label("Armor Type:");
                egui::ComboBox::from_id_salt("unit_armor_cb")
                    .selected_text(format!("{:?}", draft.armor))
                    .show_ui(ui, |ui| {
                        for a in ALL_ARMORS {
                            ui.selectable_value(&mut draft.armor, a, format!("{a:?}"));
                        }
                    });
                ui.label("Supply Cost:");
                ui.add(egui::DragValue::new(&mut draft.supply_cost).range(0..=20));
            });
            ui.add(egui::Slider::new(&mut draft.vision_range, 2.0..=32.0).text("Vision Range"));
            ui.add(egui::Slider::new(&mut draft.build_limit, 0..=32).text("Build Limit (0=∞)"));
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut draft.crushes_infantry, "Crushes Infantry");
                ui.checkbox(&mut draft.crushes_vehicles, "Crushes Vehicles");
                ui.checkbox(&mut draft.uncrushable, "Uncrushable");
                ui.checkbox(&mut draft.spy_infiltrator, "Spy Infiltrator");
                ui.checkbox(&mut draft.plants_c4, "Plants C4");
                ui.checkbox(&mut draft.immune_to_mind_control, "Immune MindCtrl");
                ui.checkbox(&mut draft.immune_to_radiation, "Immune Rad");
            });
            edit_csv_inline(
                ui,
                ("u_subf", uid),
                "Sub-Factions (CSV):",
                &mut draft.sub_factions,
            );
            edit_resource_map(
                ui,
                "Multi-Resource Costs (`resource_costs`)",
                "unit_res_costs",
                &mut draft.resource_costs,
                &resource_ids,
                &mut state.new_res_cost_key,
                &mut state.new_res_cost_val,
            );
            edit_resource_map(
                ui,
                "Passive Trickle (`resource_trickle_per_sec`)",
                "unit_res_trickle",
                &mut draft.resource_trickle_per_sec,
                &resource_ids,
                &mut state.new_res_trickle_key,
                &mut state.new_res_trickle_val,
            );
        });

    egui::CollapsingHeader::new("Movement & Locomotor Physics")
        .default_open(true)
        .show(ui, |ui| {
            ui.add(egui::Slider::new(&mut draft.speed, 0.0..=28.0).text("Top Speed"));
            ui.horizontal(|ui| {
                ui.label("Domain:");
                egui::ComboBox::from_id_salt("unit_domain_cb")
                    .selected_text(format!("{:?}", draft.movement_domain))
                    .show_ui(ui, |ui| {
                        for d in ALL_DOMAINS {
                            ui.selectable_value(&mut draft.movement_domain, d, format!("{d:?}"));
                        }
                    });
                ui.label("LocomotorKind:");
                let cur_loco = draft
                    .locomotor
                    .kind
                    .map(|k| format!("{k:?}"))
                    .unwrap_or_else(|| "Auto (Inferred)".to_string());
                egui::ComboBox::from_id_salt("unit_loco_cb")
                    .selected_text(cur_loco)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.locomotor.kind, None, "Auto (Inferred)");
                        for k in ALL_LOCOMOTORS {
                            ui.selectable_value(
                                &mut draft.locomotor.kind,
                                Some(k),
                                format!("{k:?}"),
                            );
                        }
                    });
            });
            ui.add(
                egui::Slider::new(&mut draft.locomotor.turn_rate_deg_per_sec, 0.0..=720.0)
                    .text("Turn Rate (deg/s)"),
            );
            ui.add(
                egui::Slider::new(&mut draft.locomotor.turret_slew_deg_per_sec, 0.0..=720.0)
                    .text("Turret Slew (deg/s)"),
            );
            ui.add(egui::Slider::new(&mut draft.locomotor.accel, 0.0..=60.0).text("Accel"));
            ui.add(egui::Slider::new(&mut draft.locomotor.decel, 0.0..=60.0).text("Decel"));
            ui.add(
                egui::Slider::new(&mut draft.locomotor.attack_run_radius, 0.0..=24.0)
                    .text("Attack Run Radius"),
            );
            ui.add(
                egui::Slider::new(&mut draft.cruise_altitude, 0.0..=14.0).text("Cruise Altitude"),
            );
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut draft.immobile_when_deployed, "Immobile Deployed");
                ui.checkbox(&mut draft.lands_when_deployed, "Lands Deployed");
                ui.checkbox(&mut draft.subterranean, "Subterranean");
                ui.checkbox(&mut draft.chrono_locomotion, "Chrono Locomotion");
            });
            edit_optional_ref_dropdown(
                ui,
                "Transforms Into Building:",
                "unit_trans_bldg",
                &mut draft.transforms_into_building,
                &building_ids,
            );
        });

    egui::CollapsingHeader::new("Weapons, VFX Bindings & Prerequisites")
        .default_open(true)
        .show(ui, |ui| {
            edit_optional_ref_dropdown(
                ui,
                "Primary Weapon:",
                "unit_wpn_pri",
                &mut draft.primary_weapon,
                &weapon_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Secondary Weapon:",
                "unit_wpn_sec",
                &mut draft.secondary_weapon,
                &weapon_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Elite Weapon:",
                "unit_wpn_eli",
                &mut draft.elite_weapon,
                &weapon_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Deploy Weapon:",
                "unit_wpn_dep",
                &mut draft.deploy_weapon,
                &weapon_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Ambient VFX (`ambient_vfx_id`):",
                "unit_amb_vfx",
                &mut draft.ambient_vfx_id,
                &vfx_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Death VFX (`death_vfx_id`):",
                "unit_dth_vfx",
                &mut draft.death_vfx_id,
                &vfx_ids,
            );
            ui.horizontal(|ui| {
                ui.label("Max Ammo (0=∞):");
                ui.add(egui::DragValue::new(&mut draft.max_ammo).range(0..=32));
                ui.label("Self-Heal/s:");
                ui.add(
                    egui::DragValue::new(&mut draft.self_heal_per_sec)
                        .range(0.0..=200.0)
                        .speed(0.5),
                );
            });
            edit_string_list_dropdown(
                ui,
                "Prerequisites:",
                "unit_prereqs",
                &mut draft.prerequisites,
                &building_ids,
                &mut state.new_prereq_buf,
            );
        });

    show_modular_traits_editor(
        ui,
        uid,
        &mut draft.builder_trait,
        &mut draft.gatherer_trait,
        &mut draft.squad_trait,
        &building_ids,
        &resource_ids,
        &weapon_ids,
        &mut state.new_buildable_buf,
    );

    show_shared_combat_specs_and_visuals(
        ui,
        ("unit_ext", uid),
        &mut draft.abilities,
        &mut draft.container,
        &mut draft.stealth,
        &mut draft.support_aura,
        &mut draft.point_defense,
        &mut draft.death_effect,
        &mut draft.visual,
        &mut draft.script_hooks,
        &unit_ids,
        &vfx_ids,
    );

    if draft != before {
        loaded.units.insert(uid.to_string(), draft.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_units.insert(uid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditUnitDef {
            id: uid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }
}
