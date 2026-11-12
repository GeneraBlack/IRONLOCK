use super::SchemaKind;
use crate::input::UiAssetCache;
use crate::rendering::MapTerrainDirty;
use crate::studio::dock_layout::{focus_or_open_tab, StudioTab};
use crate::studio::{SdkInspectTarget, StudioState};
use anyhow::{bail, Result};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    AiProfileDef, ArmorType, BuildingDef, BuildingRole, CampaignDef, CommanderPowerTreeDef,
    FactionDef, MissionDef, ResourceTypeDef, UnitDef, UnitRole, UpgradeDef, VfxEffectDef,
    VfxEmitterDef, VisualArchetype, VisualDef, WeaponDef,
};
use rts_simulation::world::SimWorld;
use std::path::Path;

/// Creates a valid new definition across any of the SDK schemas, persists it into the mod's
/// canonical TOML catalog (and generates a valid binary `.glb` for `Unit`/`Building`), and returns
/// the `SdkInspectTarget` to focus in the Property Inspector.
pub fn create_new_schema_entry(
    loaded: &mut LoadedMod,
    kind: SchemaKind,
    raw_id: &str,
    raw_name: &str,
    raw_faction: &str,
) -> Result<SdkInspectTarget> {
    let id = raw_id.trim().to_lowercase().replace([' ', '-'], "_");
    if id.is_empty() {
        bail!("Schema ID cannot be empty");
    }
    let display_name = if raw_name.trim().is_empty() {
        id.clone()
    } else {
        raw_name.trim().to_string()
    };
    let faction = if !raw_faction.trim().is_empty() {
        raw_faction.trim().to_string()
    } else {
        loaded
            .factions
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "allies".to_string())
    };

    match kind {
        SchemaKind::Unit => {
            rts_asset_pipeline::scaffold::generate_scaffold_model(&loaded.root_dir, &id)?;
            let unit = UnitDef {
                id: id.clone(),
                name: display_name,
                faction,
                role: UnitRole::Vehicle,
                cost: 600,
                build_time_secs: 6.0,
                health: 320.0,
                armor: ArmorType::Medium,
                speed: 5.5,
                vision_range: 16.0,
                visual: VisualDef {
                    model: format!("assets/models/{id}.glb"),
                    archetype: VisualArchetype::Tank,
                    scale: [1.5, 1.0, 2.0],
                    primary_color: [0.32, 0.58, 0.88],
                    ..Default::default()
                },
                ..Default::default()
            };
            loaded.upsert_unit_def(unit)?;
            Ok(SdkInspectTarget::Unit(id))
        }
        SchemaKind::Building => {
            rts_asset_pipeline::scaffold::generate_scaffold_model(&loaded.root_dir, &id)?;
            let building = BuildingDef {
                id: id.clone(),
                name: display_name,
                faction,
                role: BuildingRole::Support,
                cost: 800,
                build_time_secs: 8.0,
                health: 950.0,
                armor: ArmorType::Structure,
                footprint: [2, 2],
                power_delta: -25,
                vision_range: 18.0,
                visual: VisualDef {
                    model: format!("assets/models/{id}.glb"),
                    archetype: VisualArchetype::GenericBox,
                    scale: [2.0, 1.6, 2.0],
                    primary_color: [0.42, 0.62, 0.85],
                    ..Default::default()
                },
                ..Default::default()
            };
            loaded.upsert_building_def(building)?;
            Ok(SdkInspectTarget::Building(id))
        }
        SchemaKind::Weapon => {
            let weapon = WeaponDef {
                id: id.clone(),
                name: display_name,
                damage: 45.0,
                range: 14.0,
                cooldown_secs: 1.5,
                projectile_speed: 32.0,
                ..Default::default()
            };
            loaded.upsert_weapon_def(weapon)?;
            Ok(SdkInspectTarget::Weapon(id))
        }
        SchemaKind::VfxEffect => {
            let eff = VfxEffectDef {
                id: id.clone(),
                name: display_name,
                duration_secs: 1.2,
                looping: false,
                emitters: vec![VfxEmitterDef::default()],
                ..Default::default()
            };
            loaded.upsert_vfx_effect_def(eff)?;
            Ok(SdkInspectTarget::None)
        }
        SchemaKind::Upgrade => {
            let upg = UpgradeDef {
                id: id.clone(),
                name: display_name,
                faction,
                cost: 1000,
                research_time_secs: 12.0,
                damage_bonus_mult: 1.15,
                ..Default::default()
            };
            loaded.upsert_upgrade_def(upg)?;
            Ok(SdkInspectTarget::Upgrade(id))
        }
        SchemaKind::CommanderTree => {
            let tree = CommanderPowerTreeDef {
                id: id.clone(),
                faction,
                display_name,
                nodes: Vec::new(),
            };
            loaded.upsert_commander_tree_def(tree)?;
            Ok(SdkInspectTarget::CommanderTree(id))
        }
        SchemaKind::Faction => {
            let fac = FactionDef {
                id: id.clone(),
                name: display_name,
                color: [0.28, 0.68, 0.92],
                starting_credits: 10_000,
                description: "Custom playable faction.".to_string(),
                ..Default::default()
            };
            loaded.upsert_faction_def(fac)?;
            Ok(SdkInspectTarget::Faction(id))
        }
        SchemaKind::Resource => {
            let res = ResourceTypeDef {
                id: id.clone(),
                display_name,
                starting_amount: 1_000,
                color_rgb: [0.35, 0.90, 0.55],
                icon: None,
                is_capacity_pool: false,
            };
            loaded.upsert_resource_type_def(res)?;
            Ok(SdkInspectTarget::Resource(id))
        }
        SchemaKind::AiProfile => {
            let prof = AiProfileDef {
                id: id.clone(),
                faction,
                personality: "balanced".to_string(),
                ..Default::default()
            };
            loaded.upsert_ai_profile_def(prof)?;
            Ok(SdkInspectTarget::AiProfile(id))
        }
        SchemaKind::Campaign => {
            let map_id = loaded.entry_map.id.clone();
            let camp = CampaignDef {
                id: id.clone(),
                display_name: display_name.clone(),
                faction,
                description: format!("{display_name} campaign."),
                missions: vec![MissionDef {
                    id: format!("{id}_m01"),
                    display_name: "Mission 1".to_string(),
                    map_id,
                    briefing: "Establish base operations and secure the sector.".to_string(),
                    next_mission_id: None,
                    carry_over_credits_ratio: 0.25,
                    carry_over_max_veteran_units: 4,
                    loading_screen: None,
                    intro_video: None,
                    victory_video: None,
                    radar_video: None,
                    ..Default::default()
                }],
            };
            loaded.upsert_campaign_def(camp)?;
            Ok(SdkInspectTarget::Campaign(id))
        }
        SchemaKind::Map => {
            let cfg = rts_simulation::map_gen::MapGenConfig {
                seed: 2026,
                width: 64,
                height: 64,
                player_count: 2,
                ..Default::default()
            };
            let mut map_def = rts_simulation::map_gen::generate_procedural_map(&cfg, loaded);
            map_def.id = id.clone();
            map_def.name = display_name;
            let out_path = loaded.root_dir.join("maps").join(format!("{id}.toml"));
            map_def.save_to_file(&out_path)?;
            loaded.maps.insert(id, map_def);
            Ok(SdkInspectTarget::None)
        }
    }
}

/// Deletes an entry by `id` from both disk catalog TOML and `loaded` in memory.
pub fn execute_schema_delete(loaded: &mut LoadedMod, kind: SchemaKind, id: &str) -> Result<()> {
    match kind {
        SchemaKind::Unit => {
            loaded.delete_unit_def(id)?;
        }
        SchemaKind::Building => {
            loaded.delete_building_def(id)?;
        }
        SchemaKind::Weapon => {
            loaded.delete_weapon_def(id)?;
        }
        SchemaKind::VfxEffect => {
            loaded.delete_vfx_effect_def(id)?;
        }
        SchemaKind::Upgrade => {
            loaded.delete_upgrade_def(id)?;
        }
        SchemaKind::CommanderTree => {
            loaded.delete_commander_tree_def(id)?;
        }
        SchemaKind::Faction => {
            loaded.delete_faction_def(id)?;
        }
        SchemaKind::Resource => {
            loaded.delete_resource_type_def(id)?;
        }
        SchemaKind::AiProfile => {
            loaded.delete_ai_profile_def(id)?;
        }
        SchemaKind::Campaign => {
            loaded.delete_campaign_def(id)?;
        }
        SchemaKind::Map => {
            if loaded.maps.len() <= 1 {
                bail!("Cannot delete the last remaining map in the mod");
            }
            loaded.maps.remove(id);
            let map_file = loaded.root_dir.join("maps").join(format!("{id}.toml"));
            if map_file.exists() {
                std::fs::remove_file(&map_file)?;
            }
            if loaded.entry_map.id == id {
                if let Some(first_other) = loaded.maps.values().next().cloned() {
                    loaded.entry_map = first_other;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn render_inline_wizards(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    ui_cache: &mut UiAssetCache,
    terrain_dirty: &mut MapTerrainDirty,
    workspace_root: &Path,
) {
    if state.wizard_new_mod_open {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong("Scaffold New Mod Project");
            ui.horizontal(|ui| {
                ui.label("Mod ID:");
                ui.text_edit_singleline(&mut state.new_mod_id);
            });
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut state.new_mod_name);
            });
            ui.horizontal(|ui| {
                ui.label("Extends (optional):");
                ui.text_edit_singleline(&mut state.new_mod_extends);
            });
            ui.horizontal(|ui| {
                if ui.button("Create & Open Mod").clicked() {
                    let mod_id = state.new_mod_id.trim().to_lowercase().replace(' ', "-");
                    if !mod_id.is_empty() {
                        let target_dir = workspace_root.join("mods").join(&mod_id);
                        let ext = (!state.new_mod_extends.trim().is_empty())
                            .then_some(state.new_mod_extends.trim());
                        match rts_asset_pipeline::scaffold::scaffold_new_mod_project(
                            &target_dir,
                            &mod_id,
                            state.new_mod_name.trim(),
                            ext,
                        ) {
                            Ok(()) => {
                                ui_cache.invalidate_disk_catalogs();
                                state.pending_switch_mod_dir = Some(target_dir.clone());
                                state.wizard_new_mod_open = false;
                                state.status_banner = format!(
                                    "Scaffolded new mod '{}' at {}",
                                    mod_id,
                                    target_dir.display()
                                );
                            }
                            Err(err) => {
                                state.status_banner = format!("Scaffold Mod error: {err}");
                            }
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    state.wizard_new_mod_open = false;
                }
            });
        });
    }

    if state.wizard_new_unit_open {
        let mut kind = state.wizard_new_schema_kind.unwrap_or(SchemaKind::Unit);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Create New SDK Schema Definition:");
                egui::ComboBox::from_id_salt("wizard_schema_kind_combo")
                    .selected_text(kind.label())
                    .show_ui(ui, |ui| {
                        for k in SchemaKind::ALL {
                            ui.selectable_value(&mut kind, k, k.label());
                        }
                    });
            });
            state.wizard_new_schema_kind = Some(kind);

            ui.horizontal(|ui| {
                ui.label("ID:");
                ui.text_edit_singleline(&mut state.new_unit_id);
            });
            ui.horizontal(|ui| {
                ui.label("Display Name:");
                ui.text_edit_singleline(&mut state.new_unit_name);
            });
            if matches!(
                kind,
                SchemaKind::Unit
                    | SchemaKind::Building
                    | SchemaKind::Upgrade
                    | SchemaKind::CommanderTree
                    | SchemaKind::AiProfile
                    | SchemaKind::Campaign
            ) {
                ui.horizontal(|ui| {
                    ui.label("Faction:");
                    egui::ComboBox::from_id_salt("wizard_schema_faction_combo")
                        .selected_text(&state.new_unit_faction)
                        .show_ui(ui, |ui| {
                            for fid in loaded.factions.keys() {
                                ui.selectable_value(&mut state.new_unit_faction, fid.clone(), fid);
                            }
                            ui.selectable_value(
                                &mut state.new_unit_faction,
                                "any".to_string(),
                                "any",
                            );
                        });
                });
            }
            ui.horizontal(|ui| {
                if ui.button(format!("Create {}", kind.label())).clicked() {
                    match create_new_schema_entry(
                        loaded,
                        kind,
                        &state.new_unit_id,
                        &state.new_unit_name,
                        &state.new_unit_faction,
                    ) {
                        Ok(target) => {
                            let _ = world.apply_mod_hot_reload(loaded);
                            state.validation_dirty = true;
                            if kind == SchemaKind::VfxEffect {
                                let clean_id = state
                                    .new_unit_id
                                    .trim()
                                    .to_lowercase()
                                    .replace([' ', '-'], "_");
                                state.vfx_studio_ui.selected_effect_id = clean_id.clone();
                                state.vfx_studio_ui.working_effect =
                                    loaded.vfx_effects.get(&clean_id).cloned();
                                focus_or_open_tab(&mut state.dock_state, StudioTab::VfxStudio);
                            } else if target != SdkInspectTarget::None {
                                state.inspect_target = target;
                                focus_or_open_tab(
                                    &mut state.dock_state,
                                    StudioTab::PropertyInspector,
                                );
                            }
                            state.wizard_new_unit_open = false;
                            state.status_banner = format!(
                                "Created & saved new {} '{}'.",
                                kind.label(),
                                state.new_unit_id.trim()
                            );
                        }
                        Err(err) => {
                            state.status_banner =
                                format!("Failed creating {}: {err}", kind.label());
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    state.wizard_new_unit_open = false;
                }
            });
        });
    }

    if state.wizard_new_map_open {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong("Create New Playable Skirmish Map");
            ui.horizontal(|ui| {
                ui.label("Map ID:");
                ui.text_edit_singleline(&mut state.new_map_id);
            });
            ui.horizontal(|ui| {
                ui.label("Map Name:");
                ui.text_edit_singleline(&mut state.new_map_name);
            });
            ui.horizontal(|ui| {
                ui.label("Size:");
                ui.add(egui::DragValue::new(&mut state.new_map_w).range(32..=128));
                ui.label("x");
                ui.add(egui::DragValue::new(&mut state.new_map_h).range(32..=128));
            });
            ui.horizontal(|ui| {
                if ui.button("Generate & Save Map").clicked() {
                    let mid = state.new_map_id.trim().to_lowercase().replace(' ', "_");
                    if !mid.is_empty() {
                        let cfg = rts_simulation::map_gen::MapGenConfig {
                            seed: 2026,
                            width: state.new_map_w.clamp(32, 128),
                            height: state.new_map_h.clamp(32, 128),
                            player_count: 2,
                            symmetry: state.symmetry,
                            ..Default::default()
                        };
                        let mut map_def =
                            rts_simulation::map_gen::generate_procedural_map(&cfg, loaded);
                        map_def.id = mid.clone();
                        map_def.name = state.new_map_name.trim().to_string();
                        let maps_dir = loaded.root_dir.join("maps");
                        let _ = std::fs::create_dir_all(&maps_dir);
                        let out_path = maps_dir.join(format!("{mid}.toml"));
                        match map_def.save_to_file(&out_path) {
                            Ok(()) => {
                                loaded.entry_map = map_def.clone();
                                loaded.maps.insert(mid.clone(), map_def);
                                *world = SimWorld::from_mod(loaded);
                                terrain_dirty.0 = true;
                                state.validation_dirty = true;
                                state.wizard_new_map_open = false;
                                state.status_banner = format!(
                                    "Saved & loaded new map '{mid}' ({})",
                                    out_path.display()
                                );
                            }
                            Err(err) => {
                                state.status_banner = format!("Map save error: {err}");
                            }
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    state.wizard_new_map_open = false;
                }
            });
        });
    }

    if state.wizard_import_open {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong("Westwood (.mix/.ini/.vxl/.shp) & OpenRA (.yaml/.oramap) Importer");
            ui.horizontal(|ui| {
                ui.label("Source Path:");
                ui.text_edit_singleline(&mut state.import_source_path);
            });
            ui.horizontal(|ui| {
                ui.label("Output Dir (opt):");
                ui.text_edit_singleline(&mut state.import_output_dir);
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Run Smart Import").clicked() {
                    let src = Path::new(state.import_source_path.trim());
                    let out_opt = (!state.import_output_dir.trim().is_empty())
                        .then(|| Path::new(state.import_output_dir.trim()));
                    match rts_asset_pipeline::discovery::smart_import(src, out_opt) {
                        Ok(rep) => {
                            ui_cache.invalidate_disk_catalogs();
                            state.status_banner = format!(
                                "Imported {} ({} units, {} buildings, {} models) -> {}",
                                rep.detected_format,
                                rep.units_count,
                                rep.buildings_count,
                                rep.models_count,
                                rep.output_path.display()
                            );
                            state.wizard_import_open = false;
                        }
                        Err(err) => {
                            state.status_banner = format!("Smart Import failed: {err}");
                        }
                    }
                }
                if ui.button("Generate Demo Westwood + OpenRA Packs").clicked() {
                    match rts_asset_pipeline::discovery::generate_and_convert_demo_packs(
                        workspace_root,
                    ) {
                        Ok((ra2_rep, ora_rep)) => {
                            ui_cache.invalidate_disk_catalogs();
                            state.status_banner = format!(
                                "Generated demo mods: '{}' ({} units) & '{}' ({} units).",
                                ra2_rep.output_path.display(),
                                ra2_rep.units_count,
                                ora_rep.output_path.display(),
                                ora_rep.units_count
                            );
                        }
                        Err(err) => {
                            state.status_banner = format!("Demo pack conversion failed: {err}");
                        }
                    }
                }
                if ui.button("Close").clicked() {
                    state.wizard_import_open = false;
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_asset_pipeline::scaffold::scaffold_new_mod_project;
    use rts_sdk::validator::validate_mod;

    #[test]
    fn creating_all_sdk_schemas_persists_to_canonical_catalogs_and_reloads_with_zero_errors() {
        let tmp = std::env::temp_dir().join(format!(
            "rts_studio_crud_roundtrip_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        scaffold_new_mod_project(&tmp, "studio-crud-mod", "Studio CRUD Mod", None)
            .expect("scaffold mod");

        let mut loaded = LoadedMod::load_from_dir(&tmp).expect("load scaffold mod");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Faction,
            "vanguard_coalition",
            "Vanguard Coalition",
            "",
        )
        .expect("create faction");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Weapon,
            "railgun_90mm",
            "90mm Railgun",
            "",
        )
        .expect("create weapon");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::VfxEffect,
            "railgun_flash_vfx",
            "Railgun Muzzle Flash",
            "",
        )
        .expect("create vfx");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Unit,
            "paladin_hover_tank",
            "Paladin Hover Tank",
            "vanguard_coalition",
        )
        .expect("create unit");
        let mut unit = loaded.units.get("paladin_hover_tank").unwrap().clone();
        unit.primary_weapon = Some("railgun_90mm".to_string());
        loaded.upsert_unit_def(unit).expect("upsert linked unit");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Building,
            "vanguard_foundry",
            "Vanguard Foundry",
            "vanguard_coalition",
        )
        .expect("create building");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Upgrade,
            "composite_plating",
            "Composite Plating",
            "vanguard_coalition",
        )
        .expect("create upgrade");
        create_new_schema_entry(
            &mut loaded,
            SchemaKind::CommanderTree,
            "vanguard_doctrine",
            "Vanguard Doctrine",
            "vanguard_coalition",
        )
        .expect("create commander tree");
        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Resource,
            "refined_crystals",
            "Refined Crystals",
            "",
        )
        .expect("create resource");
        create_new_schema_entry(
            &mut loaded,
            SchemaKind::AiProfile,
            "vanguard_spearhead",
            "Vanguard Spearhead",
            "vanguard_coalition",
        )
        .expect("create ai profile");

        create_new_schema_entry(
            &mut loaded,
            SchemaKind::Campaign,
            "vanguard_act1",
            "Vanguard Act I",
            "vanguard_coalition",
        )
        .expect("create campaign");

        let refs = loaded.find_id_references("railgun_90mm");
        assert!(
            refs.iter().any(|r| r.contains("paladin_hover_tank")),
            "Expected paladin_hover_tank reference, got: {refs:?}"
        );

        let reloaded = LoadedMod::load_from_dir(&tmp).expect("reload mod from disk");
        assert!(reloaded.factions.contains_key("vanguard_coalition"));
        assert!(reloaded.weapons.contains_key("railgun_90mm"));
        assert!(reloaded.vfx_effects.contains_key("railgun_flash_vfx"));
        assert!(reloaded.units.contains_key("paladin_hover_tank"));
        assert!(reloaded.buildings.contains_key("vanguard_foundry"));
        assert!(reloaded.upgrades.contains_key("composite_plating"));
        assert!(reloaded.commander_trees.contains_key("vanguard_doctrine"));
        assert!(reloaded.resources.contains_key("refined_crystals"));
        assert!(reloaded.ai_profiles.contains_key("vanguard_spearhead"));
        assert!(reloaded.campaigns.contains_key("vanguard_act1"));

        let report = validate_mod(&reloaded, false);
        assert_eq!(
            report.error_count(),
            0,
            "Expected 0 validation errors on reloaded mod, got: {:?}",
            report.issues
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
