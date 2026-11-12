use super::combat_and_visuals::show_shared_combat_specs_and_visuals;
use super::show_inline_validation_badges;
use super::weapons_and_widgets::{
    edit_csv_inline, edit_optional_ref_dropdown, edit_resource_map, edit_string_list_dropdown,
};
use crate::studio::dock_layout::{focus_or_open_tab, StudioTab};
use crate::studio::history::EditorCommand;
use crate::studio::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuilderTraitSpec, BuildingDef, BuildingRole, GathererTraitSpec, SquadTraitSpec,
    SuperweaponType, SupportPowerDef, SupportPowerKind,
};
use rts_simulation::world::SimWorld;

const ALL_BUILDING_ROLES: [BuildingRole; 14] = [
    BuildingRole::ConstructionYard,
    BuildingRole::PowerPlant,
    BuildingRole::Barracks,
    BuildingRole::Refinery,
    BuildingRole::WarFactory,
    BuildingRole::Defense,
    BuildingRole::TechCenter,
    BuildingRole::Wall,
    BuildingRole::Gate,
    BuildingRole::Bridge,
    BuildingRole::BridgeRepairHut,
    BuildingRole::Airfield,
    BuildingRole::ServiceDepot,
    BuildingRole::Support,
];

const ALL_SUPERWEAPONS: [SuperweaponType; 7] = [
    SuperweaponType::NuclearMissile,
    SuperweaponType::WeatherStorm,
    SuperweaponType::IronCurtain,
    SuperweaponType::Chronosphere,
    SuperweaponType::Paratroopers,
    SuperweaponType::GeneticMutator,
    SuperweaponType::PsychicDominator,
];

const ALL_SUPPORT_POWERS: [SupportPowerKind; 11] = [
    SupportPowerKind::Paradrop,
    SupportPowerKind::RadarScan,
    SupportPowerKind::Airstrike,
    SupportPowerKind::ChronoShift,
    SupportPowerKind::EmpPulse,
    SupportPowerKind::ForceShield,
    SupportPowerKind::NuclearStrike,
    SupportPowerKind::LightningStorm,
    SupportPowerKind::IronCurtain,
    SupportPowerKind::GeneticMutator,
    SupportPowerKind::PsychicDominator,
];

pub(super) fn show_building_def_inspector(
    ui: &mut egui::Ui,
    bid: &str,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let Some(before) = loaded.buildings.get(bid).cloned() else {
        ui.weak(format!("Building '{bid}' not found in active mod."));
        return;
    };
    let mut draft: BuildingDef = before.clone();
    let faction_ids: Vec<String> = loaded.factions.keys().cloned().collect();
    let weapon_ids: Vec<String> = loaded.weapons.keys().cloned().collect();
    let building_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
    let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
    let resource_ids: Vec<String> = loaded.resources.keys().cloned().collect();
    let vfx_ids: Vec<String> = loaded.vfx_effects.keys().cloned().collect();

    ui.horizontal_wrapped(|ui| {
        ui.heading(format!("BuildingDef: {bid}"));
        if ui.button("💾 Save to .toml & Hot-Reload").clicked() {
            match loaded.save_building_def_toml(draft.clone()) {
                Ok(path) => {
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.modified_buildings.remove(bid);
                    state.validation_dirty = true;
                    state.status_banner = format!("Saved Building '{bid}' -> {}", path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save error for '{bid}': {err}");
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
    show_inline_validation_badges(ui, state, bid);

    egui::CollapsingHeader::new("Structure Core, Upgrades & Addon Sockets")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Display Name:");
                ui.text_edit_singleline(&mut draft.name);
            });
            ui.horizontal(|ui| {
                ui.label("Faction:");
                egui::ComboBox::from_id_salt("bldg_faction_cb")
                    .selected_text(&draft.faction)
                    .show_ui(ui, |ui| {
                        for fid in &faction_ids {
                            ui.selectable_value(&mut draft.faction, fid.clone(), fid);
                        }
                    });
                ui.label("Role:");
                egui::ComboBox::from_id_salt("bldg_role_cb")
                    .selected_text(format!("{:?}", draft.role))
                    .show_ui(ui, |ui| {
                        for r in ALL_BUILDING_ROLES {
                            ui.selectable_value(&mut draft.role, r, format!("{r:?}"));
                        }
                    });
            });
            ui.add(egui::Slider::new(&mut draft.cost, 0..=10000).text("Credit Cost ($)"));
            ui.add(
                egui::Slider::new(&mut draft.build_time_secs, 0.5..=240.0).text("Build Time (s)"),
            );
            ui.add(egui::Slider::new(&mut draft.health, 50.0..=20000.0).text("Max Health"));
            ui.add(egui::Slider::new(&mut draft.power_delta, -500..=800).text("Power Delta (MW)"));
            ui.horizontal_wrapped(|ui| {
                ui.label("Footprint:");
                ui.add(egui::DragValue::new(&mut draft.footprint[0]).range(1..=6));
                ui.label("x");
                ui.add(egui::DragValue::new(&mut draft.footprint[1]).range(1..=6));
                ui.checkbox(&mut draft.water_bound, "Water-Bound");
                ui.checkbox(&mut draft.leaves_ruin, "Leaves Ruin");
                ui.checkbox(&mut draft.cloning_vats, "Cloning Vats");
                ui.checkbox(&mut draft.grinder, "Grinder");
            });
            ui.add(egui::Slider::new(&mut draft.supply_provided, 0..=200).text("Supply Provided"));
            ui.add(egui::Slider::new(&mut draft.airfield_pads, 0..=8).text("Airfield Pads"));

            let sw_txt = draft
                .superweapon
                .map(|s| format!("{s:?}"))
                .unwrap_or_else(|| "(None)".to_string());
            ui.horizontal(|ui| {
                ui.label("Superweapon:");
                egui::ComboBox::from_id_salt(("bldg_sw_cb", bid))
                    .selected_text(sw_txt)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.superweapon, None, "(None)");
                        for sw in ALL_SUPERWEAPONS {
                            ui.selectable_value(
                                &mut draft.superweapon,
                                Some(sw),
                                format!("{sw:?}"),
                            );
                        }
                    });
            });

            edit_optional_ref_dropdown(
                ui,
                "Primary Defense Weapon:",
                "bldg_wpn_pri",
                &mut draft.primary_weapon,
                &weapon_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Ambient VFX (`ambient_vfx_id`):",
                "bldg_amb_vfx",
                &mut draft.ambient_vfx_id,
                &vfx_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Death VFX (`death_vfx_id`):",
                "bldg_dth_vfx",
                &mut draft.death_vfx_id,
                &vfx_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Spawns Unit On Complete:",
                "bldg_spawn_unit",
                &mut draft.spawns_unit_on_complete,
                &unit_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Transforms Into Unit:",
                "bldg_trans_unit",
                &mut draft.transforms_into_unit,
                &unit_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Upgrades To Building:",
                "bldg_upg_to",
                &mut draft.upgrades_to,
                &building_ids,
            );
            ui.horizontal(|ui| {
                ui.label("Upgrade Cost/$ & Time(s):");
                ui.add(egui::DragValue::new(&mut draft.upgrade_cost).range(0..=10000));
                ui.add(egui::DragValue::new(&mut draft.upgrade_time_secs).range(0.0..=180.0));
            });
            edit_csv_inline(
                ui,
                ("bldg_upg_opts", bid),
                "Upgrade Options (`upgrade_options`):",
                &mut draft.upgrade_options,
            );
            edit_csv_inline(
                ui,
                ("bldg_addons", bid),
                "Addon Sockets (`addon_sockets`):",
                &mut draft.addon_sockets,
            );
            edit_string_list_dropdown(
                ui,
                "Prerequisites:",
                "bldg_prereqs",
                &mut draft.prerequisites,
                &building_ids,
                &mut state.new_prereq_buf,
            );
            edit_string_list_dropdown(
                ui,
                "Unlocks Units:",
                "bldg_unlocks",
                &mut draft.unlocks_units,
                &unit_ids,
                &mut state.new_unlock_unit_buf,
            );
            edit_resource_map(
                ui,
                "Multi-Resource Costs (`resource_costs`)",
                "bldg_res_costs",
                &mut draft.resource_costs,
                &resource_ids,
                &mut state.new_res_cost_key,
                &mut state.new_res_cost_val,
            );
            edit_resource_map(
                ui,
                "Passive Trickle (`resource_trickle_per_sec`)",
                "bldg_res_trickle",
                &mut draft.resource_trickle_per_sec,
                &resource_ids,
                &mut state.new_res_trickle_key,
                &mut state.new_res_trickle_val,
            );
        });

    egui::CollapsingHeader::new(format!(
        "Support Powers (`support_powers`: {})",
        draft.support_powers.len()
    ))
    .default_open(!draft.support_powers.is_empty())
    .show(ui, |ui| {
        if ui.small_button("+ Add SupportPowerDef").clicked() {
            let n = draft.support_powers.len() + 1;
            draft.support_powers.push(SupportPowerDef {
                id: format!("{bid}_power_{n}"),
                name: format!("Support Power {n}"),
                ..Default::default()
            });
        }
        let mut rem_sp = None;
        for (idx, sp) in draft.support_powers.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut sp.id)
                            .id_salt(("sp_id", bid, idx))
                            .desired_width(110.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut sp.name)
                            .id_salt(("sp_nm", bid, idx))
                            .desired_width(125.0),
                    );
                    egui::ComboBox::from_id_salt(("sp_kind", bid, idx))
                        .selected_text(format!("{:?}", sp.kind))
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for k in ALL_SUPPORT_POWERS {
                                ui.selectable_value(&mut sp.kind, k, format!("{k:?}"));
                            }
                        });
                    if ui.small_button("✖").clicked() {
                        rem_sp = Some(idx);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut sp.cooldown_secs)
                            .range(1.0..=600.0)
                            .speed(1.0)
                            .prefix("CD(s): "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sp.radius)
                            .range(0.5..=32.0)
                            .speed(0.5)
                            .prefix("Rad: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sp.damage)
                            .range(0.0..=10000.0)
                            .speed(10.0)
                            .prefix("Dmg: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sp.duration_secs)
                            .range(0.0..=120.0)
                            .speed(0.5)
                            .prefix("Dur: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sp.count)
                            .range(1..=30)
                            .prefix("Cnt: "),
                    );
                });
                edit_optional_ref_dropdown(
                    ui,
                    "Spawn/Delivery Unit:",
                    &format!("sp_unit_{bid}_{idx}"),
                    &mut sp.unit_id,
                    &unit_ids,
                );
            });
        }
        if let Some(i) = rem_sp {
            draft.support_powers.remove(i);
        }
    });

    show_modular_traits_editor(
        ui,
        bid,
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
        ("bldg_ext", bid),
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
        loaded.buildings.insert(bid.to_string(), draft.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_buildings.insert(bid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditBuildingDef {
            id: bid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn show_modular_traits_editor(
    ui: &mut egui::Ui,
    owner_id: &str,
    builder_trait: &mut Option<BuilderTraitSpec>,
    gatherer_trait: &mut Option<GathererTraitSpec>,
    squad_trait: &mut Option<SquadTraitSpec>,
    building_ids: &[String],
    resource_ids: &[String],
    weapon_ids: &[String],
    new_buf: &mut String,
) {
    egui::CollapsingHeader::new("Modular Traits (Builder / Gatherer / Squad)")
        .id_salt(("mod_traits_hdr", owner_id))
        .default_open(false)
        .show(ui, |ui| {
            let mut has_builder = builder_trait.is_some();
            if ui
                .checkbox(&mut has_builder, "Enable BuilderTraitSpec (Worker / Dozer)")
                .changed()
            {
                *builder_trait = has_builder.then(|| BuilderTraitSpec {
                    buildable_buildings: building_ids.iter().take(3).cloned().collect(),
                    ..Default::default()
                });
            }
            if let Some(bt) = builder_trait.as_mut() {
                ui.add(
                    egui::Slider::new(&mut bt.build_speed_multiplier, 0.2..=4.0)
                        .text("Build Speed Mult"),
                );
                ui.add(egui::Slider::new(&mut bt.build_range, 1.5..=10.0).text("Build Range"));
                ui.checkbox(&mut bt.can_repair, "Can Repair Friendly Structures");
                ui.add(
                    egui::Slider::new(&mut bt.repair_hp_per_sec, 0.0..=150.0).text("Repair HP/sec"),
                );
                ui.checkbox(&mut bt.consumed_on_complete, "Consumed On Complete");
                edit_string_list_dropdown(
                    ui,
                    "Buildable Buildings:",
                    &format!("bt_bldgs_{owner_id}"),
                    &mut bt.buildable_buildings,
                    building_ids,
                    new_buf,
                );
            }

            ui.separator();

            let mut has_gatherer = gatherer_trait.is_some();
            if ui
                .checkbox(
                    &mut has_gatherer,
                    "Enable GathererTraitSpec (Resource Harvester)",
                )
                .changed()
            {
                *gatherer_trait = has_gatherer.then(GathererTraitSpec::default);
            }
            if let Some(gt) = gatherer_trait.as_mut() {
                ui.add(egui::Slider::new(&mut gt.carry_capacity, 50..=5000).text("Carry Capacity"));
                ui.add(
                    egui::Slider::new(&mut gt.gather_rate_per_sec, 5.0..=500.0)
                        .text("Gather Rate / sec"),
                );
                edit_string_list_dropdown(
                    ui,
                    "Allowed Resources (`allowed_resources`):",
                    &format!("gt_res_{owner_id}"),
                    &mut gt.allowed_resources,
                    resource_ids,
                    new_buf,
                );
                edit_string_list_dropdown(
                    ui,
                    "Dropoff Buildings (`dropoff_buildings`):",
                    &format!("gt_drop_{owner_id}"),
                    &mut gt.dropoff_buildings,
                    building_ids,
                    new_buf,
                );
            }

            ui.separator();

            let mut has_squad = squad_trait.is_some();
            if ui
                .checkbox(
                    &mut has_squad,
                    "Enable SquadTraitSpec (Multi-Member Squad & Cover)",
                )
                .changed()
            {
                *squad_trait = has_squad.then(SquadTraitSpec::default);
            }
            if let Some(sq) = squad_trait.as_mut() {
                ui.add(egui::Slider::new(&mut sq.member_count, 2..=12).text("Squad Members"));
                ui.add(egui::Slider::new(&mut sq.member_spacing, 0.2..=1.8).text("Member Spacing"));
                ui.add(
                    egui::Slider::new(&mut sq.reinforce_cost_ratio, 0.1..=1.0)
                        .text("Reinforce Cost Ratio"),
                );
                ui.add(
                    egui::Slider::new(&mut sq.directional_cover_multiplier, 0.2..=1.0)
                        .text("Cover Damage Mult"),
                );
                ui.add(
                    egui::Slider::new(&mut sq.max_suppression, 10.0..=300.0)
                        .text("Max Suppression"),
                );
                ui.horizontal_wrapped(|ui| {
                    let mut lc = sq.light_cover_damage_mult.unwrap_or(0.75);
                    if ui
                        .add(
                            egui::DragValue::new(&mut lc)
                                .range(0.1..=1.0)
                                .speed(0.02)
                                .prefix("LightCov: "),
                        )
                        .changed()
                    {
                        sq.light_cover_damage_mult = Some(lc);
                    }
                    let mut hc = sq.heavy_cover_damage_mult.unwrap_or(0.45);
                    if ui
                        .add(
                            egui::DragValue::new(&mut hc)
                                .range(0.1..=1.0)
                                .speed(0.02)
                                .prefix("HeavyCov: "),
                        )
                        .changed()
                    {
                        sq.heavy_cover_damage_mult = Some(hc);
                    }
                    let mut pin = sq.pin_suppression_threshold.unwrap_or(80.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut pin)
                                .range(5.0..=300.0)
                                .speed(1.0)
                                .prefix("Pin@: "),
                        )
                        .changed()
                    {
                        sq.pin_suppression_threshold = Some(pin);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let mut auto_reinf = sq.auto_reinforce_default.unwrap_or(false);
                    if ui
                        .checkbox(&mut auto_reinf, "Auto-Reinforce Default")
                        .changed()
                    {
                        sq.auto_reinforce_default = Some(auto_reinf);
                    }
                    ui.label("Weapon Upgrade Slots:");
                    ui.add(egui::DragValue::new(&mut sq.weapon_upgrade_slots).range(0..=4));
                });
                edit_string_list_dropdown(
                    ui,
                    "Available Weapon Upgrades:",
                    &format!("sq_wupg_{owner_id}"),
                    &mut sq.available_weapon_upgrades,
                    weapon_ids,
                    new_buf,
                );
            }
        });
}
