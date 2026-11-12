use super::combat_and_visuals::edit_status_effect_compact;
use super::{show_inline_validation_badges, ALL_ARMORS};
use crate::input::ClientInteractionState;
use crate::studio::dock_layout::{focus_or_open_tab, StudioTab};
use crate::studio::history::EditorCommand;
use crate::studio::{SdkInspectTarget, StudioState};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{DeliveryKind, HazardZoneSpec, ProjectileStyle, StatusEffectDef, WeaponDef};
use rts_simulation::command::UnitStance;
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeMap;

const ALL_DELIVERIES: [DeliveryKind; 3] = [
    DeliveryKind::Instant,
    DeliveryKind::Ballistic,
    DeliveryKind::Missile,
];

const ALL_PROJ_STYLES: [ProjectileStyle; 7] = [
    ProjectileStyle::Bullet,
    ProjectileStyle::Laser,
    ProjectileStyle::Tesla,
    ProjectileStyle::Radiation,
    ProjectileStyle::Sonic,
    ProjectileStyle::MindControl,
    ProjectileStyle::Missile,
];

pub(super) fn show_weapon_def_inspector(
    ui: &mut egui::Ui,
    wid: &str,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let Some(before) = loaded.weapons.get(wid).cloned() else {
        ui.weak(format!("Weapon '{wid}' not found in active mod."));
        return;
    };
    let mut draft: WeaponDef = before.clone();
    let vfx_ids: Vec<String> = loaded.vfx_effects.keys().cloned().collect();

    ui.horizontal_wrapped(|ui| {
        ui.heading(format!("WeaponDef: {wid}"));
        if ui.button("💾 Save to .toml & Hot-Reload").clicked() {
            match loaded.save_weapon_def_toml(draft.clone()) {
                Ok(path) => {
                    let _ = world.apply_mod_hot_reload(loaded);
                    state.modified_weapons.remove(wid);
                    state.validation_dirty = true;
                    state.status_banner = format!("Saved Weapon '{wid}' -> {}", path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save error for '{wid}': {err}");
                }
            }
        }
        if ui.small_button("✨ VFX & Projectile Studio").clicked() {
            state.vfx_studio_ui.assign_weapon_id = wid.to_string();
            focus_or_open_tab(&mut state.dock_state, StudioTab::VfxStudio);
        }
    });
    show_inline_validation_badges(ui, state, wid);

    egui::CollapsingHeader::new("Damage, Range, Delivery & VFX Bindings")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Display Name:");
                ui.text_edit_singleline(&mut draft.name);
            });
            ui.add(egui::Slider::new(&mut draft.damage, 0.0..=5000.0).text("Base Damage"));
            ui.add(egui::Slider::new(&mut draft.range, 0.5..=48.0).text("Max Range"));
            ui.add(egui::Slider::new(&mut draft.min_range, 0.0..=18.0).text("Min Range"));
            ui.add(egui::Slider::new(&mut draft.cooldown_secs, 0.05..=25.0).text("Cooldown (s)"));
            ui.add(egui::Slider::new(&mut draft.splash_radius, 0.0..=12.0).text("Splash Radius"));
            ui.add(egui::Slider::new(&mut draft.burst, 1..=12).text("Burst Shots"));
            ui.horizontal_wrapped(|ui| {
                ui.label("Delivery:");
                egui::ComboBox::from_id_salt("wpn_delivery_cb")
                    .selected_text(format!("{:?}", draft.delivery))
                    .show_ui(ui, |ui| {
                        for d in ALL_DELIVERIES {
                            ui.selectable_value(&mut draft.delivery, d, format!("{d:?}"));
                        }
                    });
                ui.label("Style:");
                egui::ComboBox::from_id_salt("wpn_style_cb")
                    .selected_text(format!("{:?}", draft.projectile_style))
                    .show_ui(ui, |ui| {
                        for st in ALL_PROJ_STYLES {
                            ui.selectable_value(&mut draft.projectile_style, st, format!("{st:?}"));
                        }
                    });
                ui.checkbox(&mut draft.targets_ground, "Ground");
                ui.checkbox(&mut draft.targets_air, "Air");
                ui.checkbox(&mut draft.friendly_fire, "Friendly Fire");
            });
            ui.add(
                egui::Slider::new(&mut draft.projectile_speed, 1.0..=80.0).text("Projectile Speed"),
            );
            ui.add(egui::Slider::new(&mut draft.arc_height, 0.0..=14.0).text("Arc Height"));
            edit_optional_ref_dropdown(
                ui,
                "Muzzle VFX (`muzzle_vfx_id`):",
                "wpn_muz_vfx",
                &mut draft.muzzle_vfx_id,
                &vfx_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Projectile VFX (`projectile_vfx_id`):",
                "wpn_prj_vfx",
                &mut draft.projectile_vfx_id,
                &vfx_ids,
            );
            edit_optional_ref_dropdown(
                ui,
                "Impact VFX (`impact_vfx_id`):",
                "wpn_imp_vfx",
                &mut draft.impact_vfx_id,
                &vfx_ids,
            );
        });

    egui::CollapsingHeader::new("Ballistic Physics (`BallisticSpec`)")
        .default_open(true)
        .show(ui, |ui| {
            ui.add(
                egui::Slider::new(&mut draft.ballistics.gravity, 0.0..=40.0).text("Gravity (m/s²)"),
            );
            ui.add(
                egui::Slider::new(&mut draft.ballistics.scatter_radius, 0.0..=6.0)
                    .text("Scatter Radius"),
            );
            ui.add(
                egui::Slider::new(&mut draft.ballistics.moving_scatter_multiplier, 1.0..=4.0)
                    .text("Moving Scatter Mult"),
            );
            ui.add(
                egui::Slider::new(&mut draft.ballistics.lead_factor, 0.0..=1.5)
                    .text("Target Lead Factor"),
            );
            ui.add(
                egui::Slider::new(&mut draft.ballistics.suppression_amount, 0.0..=100.0)
                    .text("Squad Suppression"),
            );
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut draft.ballistics.collides_with_terrain,
                    "Terrain Occlusion",
                );
                ui.checkbox(
                    &mut draft.ballistics.collides_with_structures,
                    "Structure Occlusion",
                );
            });
        });

    egui::CollapsingHeader::new("On-Hit Status Effects, Hazard Zones & Exotic Warheads")
        .default_open(
            draft.on_hit_status.is_some()
                || !draft.on_hit_statuses.is_empty()
                || draft.on_hit_zone.is_some(),
        )
        .show(ui, |ui| {
            let mut has_primary_st = draft.on_hit_status.is_some();
            if ui
                .checkbox(
                    &mut has_primary_st,
                    "Primary `on_hit_status` (`StatusEffectDef`)",
                )
                .changed()
            {
                draft.on_hit_status = has_primary_st.then(StatusEffectDef::default);
            }
            if let Some(st) = draft.on_hit_status.as_mut() {
                edit_status_effect_compact(ui, ("wpn_ohs", wid), st);
            }

            ui.horizontal(|ui| {
                ui.strong(format!(
                    "Additional `on_hit_statuses` ({})",
                    draft.on_hit_statuses.len()
                ));
                if ui.small_button("+ Add StatusEffectDef").clicked() {
                    draft.on_hit_statuses.push(StatusEffectDef::default());
                }
            });
            let mut rem_st = None;
            for (idx, st) in draft.on_hit_statuses.iter_mut().enumerate() {
                ui.group(|ui| {
                    edit_status_effect_compact(ui, ("wpn_ohs_vec", wid, idx), st);
                    if ui.small_button("✖ Remove Status").clicked() {
                        rem_st = Some(idx);
                    }
                });
            }
            if let Some(i) = rem_st {
                draft.on_hit_statuses.remove(i);
            }

            ui.separator();
            let mut has_hz = draft.on_hit_zone.is_some();
            if ui
                .checkbox(
                    &mut has_hz,
                    "Spawn Lingering `HazardZoneSpec` (`on_hit_zone`)",
                )
                .changed()
            {
                draft.on_hit_zone = has_hz.then(HazardZoneSpec::default);
            }
            if let Some(hz) = draft.on_hit_zone.as_mut() {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut hz.radius)
                            .range(0.5..=20.0)
                            .speed(0.25)
                            .prefix("Rad: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut hz.dps)
                            .range(1.0..=500.0)
                            .speed(1.0)
                            .prefix("DPS: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut hz.duration_secs)
                            .range(0.5..=120.0)
                            .speed(0.5)
                            .prefix("Dur(s): "),
                    );
                    ui.checkbox(&mut hz.is_toxic_cloud, "Toxic Cloud");
                });
            }

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut draft.mind_control, "Mind Control");
                ui.checkbox(&mut draft.can_mind_control_buildings, "MC Buildings");
                ui.checkbox(&mut draft.warp_erase, "Chrono Erase");
                ui.checkbox(&mut draft.c4_demolition, "C4 Demolition");
                ui.checkbox(&mut draft.clears_garrison, "Clears Garrison");
                ui.checkbox(&mut draft.prism_support, "Prism Support");
                ui.checkbox(&mut draft.naval_only, "Naval Only");
                ui.checkbox(&mut draft.kamikaze, "Kamikaze");
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut draft.refraction_targets)
                        .range(0..=10)
                        .prefix("RefractCnt: "),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.refraction_damage_ratio)
                        .range(0.0..=2.0)
                        .speed(0.05)
                        .prefix("RefractRatio: "),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.disables_building_duration_secs)
                        .range(0.0..=120.0)
                        .prefix("EMP Bldg(s): "),
                );
                ui.add(
                    egui::DragValue::new(&mut draft.steals_credits_per_hit)
                        .range(0..=5000)
                        .prefix("Steal $: "),
                );
            });
        });

    egui::CollapsingHeader::new("Armor Multipliers")
        .default_open(false)
        .show(ui, |ui| {
            for armor in ALL_ARMORS {
                let mut mult = draft.armor_multipliers.get(&armor).copied().unwrap_or(1.0);
                if ui
                    .add(egui::Slider::new(&mut mult, 0.05..=3.5).text(format!("vs {armor:?}")))
                    .changed()
                {
                    draft.armor_multipliers.insert(armor, mult);
                }
            }
        });

    if draft != before {
        loaded.weapons.insert(wid.to_string(), draft.clone());
        let _ = world.apply_mod_hot_reload(loaded);
        state.modified_weapons.insert(wid.to_string());
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditWeaponDef {
            id: wid.to_string(),
            before: Box::new(before),
            after: Box::new(draft),
        });
    }
}

pub(super) fn edit_csv_inline(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash,
    label: &str,
    list: &mut Vec<String>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut joined = list.join(", ");
        if ui
            .add(
                egui::TextEdit::singleline(&mut joined)
                    .id_salt(salt)
                    .desired_width(165.0),
            )
            .changed()
        {
            *list = joined
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    });
}

pub(super) fn edit_optional_ref_dropdown(
    ui: &mut egui::Ui,
    label: &str,
    salt: &str,
    slot: &mut Option<String>,
    options: &[String],
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let cur = slot.as_deref().unwrap_or("(None)");
        egui::ComboBox::from_id_salt(salt)
            .selected_text(cur)
            .show_ui(ui, |ui| {
                ui.selectable_value(slot, None, "(None)");
                for opt in options {
                    ui.selectable_value(slot, Some(opt.clone()), opt);
                }
            });
    });
}

pub(super) fn edit_string_list_dropdown(
    ui: &mut egui::Ui,
    label: &str,
    salt: &str,
    list: &mut Vec<String>,
    candidates: &[String],
    selected_to_add: &mut String,
) {
    ui.label(label);
    let mut remove_idx: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        for (i, item) in list.iter().enumerate() {
            if ui.small_button(format!("{item} ✖")).clicked() {
                remove_idx = Some(i);
            }
        }
    });
    if let Some(i) = remove_idx {
        list.remove(i);
    }
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(if selected_to_add.is_empty() {
                "Pick ID..."
            } else {
                selected_to_add.as_str()
            })
            .show_ui(ui, |ui| {
                for c in candidates {
                    ui.selectable_value(selected_to_add, c.clone(), c);
                }
            });
        if ui.small_button("+ Add").clicked()
            && !selected_to_add.trim().is_empty()
            && !list.contains(selected_to_add)
        {
            list.push(selected_to_add.trim().to_string());
        }
    });
}

pub(super) fn edit_resource_map(
    ui: &mut egui::Ui,
    title: &str,
    salt: &str,
    map: &mut BTreeMap<String, u32>,
    known_resources: &[String],
    new_key: &mut String,
    new_val: &mut u32,
) {
    ui.push_id(salt, |ui| {
        ui.label(title);
        let keys: Vec<String> = map.keys().cloned().collect();
        let mut remove_key: Option<String> = None;
        for k in keys {
            if let Some(val) = map.get_mut(&k) {
                ui.horizontal(|ui| {
                    ui.label(&k);
                    ui.add(egui::DragValue::new(val).range(0..=50000));
                    if ui.small_button("✖").clicked() {
                        remove_key = Some(k.clone());
                    }
                });
            }
        }
        if let Some(rk) = remove_key {
            map.remove(&rk);
        }
        ui.horizontal(|ui| {
            if !known_resources.is_empty() {
                egui::ComboBox::from_id_salt("res_pick")
                    .selected_text(if new_key.is_empty() {
                        "Resource..."
                    } else {
                        new_key.as_str()
                    })
                    .width(95.0)
                    .show_ui(ui, |ui| {
                        for rid in known_resources {
                            ui.selectable_value(new_key, rid.clone(), rid);
                        }
                    });
            } else {
                ui.text_edit_singleline(new_key);
            }
            ui.add(egui::DragValue::new(new_val).range(0..=50000));
            if ui.small_button("+ Set").clicked() && !new_key.trim().is_empty() {
                map.insert(new_key.trim().to_string(), *new_val);
            }
        });
    });
}

pub(super) fn show_selected_map_entity_section(
    ui: &mut egui::Ui,
    sel_id: u64,
    state: &mut StudioState,
    world: &mut SimWorld,
    interaction: &mut ClientInteractionState,
) {
    let Some(ent_snapshot) = world.entities.get(&sel_id).cloned() else {
        return;
    };
    let locked = state.locked_entity_ids.contains(&sel_id);
    egui::CollapsingHeader::new(format!("Selected Map Entity #{sel_id}"))
        .default_open(true)
        .show(ui, |ui| {
            let def_label = match &ent_snapshot.kind {
                EntityKind::Unit { unit_id } => format!("Unit '{unit_id}'"),
                EntityKind::Building { building_id } => format!("Building '{building_id}'"),
            };
            ui.horizontal(|ui| {
                ui.strong(&def_label);
                if ui.small_button("Inspect SDK Def").clicked() {
                    state.inspect_target = match &ent_snapshot.kind {
                        EntityKind::Unit { unit_id } => SdkInspectTarget::Unit(unit_id.clone()),
                        EntityKind::Building { building_id } => {
                            SdkInspectTarget::Building(building_id.clone())
                        }
                    };
                }
            });

            if locked {
                ui.colored_label(
                    egui::Color32::from_rgb(240, 185, 80),
                    "🔒 Entity is locked in Scene Hierarchy.",
                );
                return;
            }

            let mut tag_buf = world.entity_tags.get(&sel_id).cloned().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Script Tag (`entity_tags`):");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut tag_buf)
                            .desired_width(145.0)
                            .hint_text("e.g. vip_convoy_1"),
                    )
                    .changed()
                {
                    let trimmed = tag_buf.trim().to_string();
                    if trimmed.is_empty() {
                        world.entity_tags.remove(&sel_id);
                    } else {
                        world.entity_tags.insert(sel_id, trimmed);
                    }
                }
            });

            let mut delete_now = false;
            if let Some(ent) = world.entities.get_mut(&sel_id) {
                ui.horizontal(|ui| {
                    ui.label("Owner Player:");
                    egui::ComboBox::from_id_salt("map_ent_owner")
                        .selected_text(format!("Player {}", ent.player_id))
                        .show_ui(ui, |ui| {
                            for pid in 0u8..=8u8 {
                                ui.selectable_value(
                                    &mut ent.player_id,
                                    pid,
                                    format!("Player {pid}"),
                                );
                            }
                        });
                });

                let max_hp = ent.max_health.max(1.0);
                ui.add(egui::Slider::new(&mut ent.health, 1.0..=max_hp).text("Health"));

                let old_pos = ent.position;
                let old_tile = ent.tile_origin;
                let old_facing = ent.facing_radians;
                let mut new_pos = old_pos;
                let mut facing_deg = old_facing.to_degrees();
                ui.horizontal(|ui| {
                    ui.label("World XYZ:");
                    ui.add(
                        egui::DragValue::new(&mut new_pos[0])
                            .speed(0.25)
                            .prefix("X: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut new_pos[1])
                            .speed(0.10)
                            .prefix("Y: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut new_pos[2])
                            .speed(0.25)
                            .prefix("Z: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut facing_deg)
                            .speed(2.0)
                            .prefix("Yaw°: "),
                    );
                });
                let new_facing = facing_deg.to_radians();
                if new_pos != old_pos || (new_facing - old_facing).abs() > 1e-4 {
                    ent.position = new_pos;
                    ent.facing_radians = new_facing;
                    ent.turret_facing_radians = new_facing;
                    state
                        .history
                        .push_executed(EditorCommand::TransformEntities {
                            label: format!("Inspector Transform #{sel_id}"),
                            before: vec![(sel_id, old_pos, old_tile, old_facing)],
                            after: vec![(sel_id, new_pos, ent.tile_origin, new_facing)],
                        });
                }

                ui.horizontal(|ui| {
                    ui.label("Stance:");
                    for st in [
                        UnitStance::Aggressive,
                        UnitStance::Defensive,
                        UnitStance::HoldGround,
                        UnitStance::HoldFire,
                    ] {
                        ui.selectable_value(&mut ent.stance, st, st.label());
                    }
                });

                if ui.button("🗑 Delete Entity").clicked() {
                    delete_now = true;
                }
            }
            if delete_now {
                world.entity_tags.remove(&sel_id);
                if let Some(removed) = world.entities.remove(&sel_id) {
                    interaction.selected_ids.remove(&sel_id);
                    state.history.push_executed(EditorCommand::DeleteEntity {
                        entity: Box::new(removed),
                    });
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sdk::schema::LocomotorKind;

    #[test]
    fn unit_def_roundtrip_save_and_hot_reload_preserves_custom_traits_and_anims() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rust_rts_inspector_roundtrip_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        rts_asset_pipeline::scaffold::scaffold_new_mod_project(
            &temp_dir,
            "inspector-test",
            "Inspector Test",
            None,
        )
        .expect("scaffold temp mod");

        let mut loaded = LoadedMod::load_from_dir(&temp_dir).expect("load temp mod");
        let uid = loaded.units.keys().next().expect("at least 1 unit").clone();
        let mut edited = loaded.units[&uid].clone();
        edited.cost += 125;
        edited.locomotor.kind = Some(LocomotorKind::Tracked);
        edited.resource_costs.insert("alloy".to_string(), 40);
        edited.stealth.detects_cloak = true;
        edited.support_aura.grants_radar = true;
        edited.visual.animations.prone = Some("ProneCrawl".to_string());
        edited.visual.animations.damaged = Some("HeavySmokeLoop".to_string());

        let saved_path = loaded
            .save_unit_def_toml(edited.clone())
            .expect("save_unit_def_toml");
        assert!(saved_path.exists());

        let reloaded = LoadedMod::load_from_dir(&temp_dir).expect("reload temp mod");
        assert_eq!(reloaded.units[&uid].cost, edited.cost);
        assert_eq!(
            reloaded.units[&uid].locomotor.kind,
            Some(LocomotorKind::Tracked)
        );
        assert!(reloaded.units[&uid].stealth.detects_cloak);
        assert!(reloaded.units[&uid].support_aura.grants_radar);
        assert_eq!(
            reloaded.units[&uid].visual.animations.prone.as_deref(),
            Some("ProneCrawl")
        );
        assert_eq!(
            reloaded.units[&uid].visual.animations.damaged.as_deref(),
            Some("HeavySmokeLoop")
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
