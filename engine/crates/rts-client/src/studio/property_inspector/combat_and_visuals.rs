use super::weapons_and_widgets::edit_optional_ref_dropdown;
use bevy_egui::egui;
use rts_sdk::schema::{
    AbilityDef, AbilityTargetMode, AnimClipMap, ContainerSpec, DeathPayloadSpec, EffectPayload,
    PointDefenseSpec, StatusEffectDef, StatusVfxStyle, StealthSensorSpec, SupportAuraSpec,
    TeamMaskMode, VisualArchetype, VisualDef,
};
use std::collections::BTreeMap;

const ALL_VISUAL_ARCHETYPES: [VisualArchetype; 12] = [
    VisualArchetype::Infantry,
    VisualArchetype::Tank,
    VisualArchetype::Harvester,
    VisualArchetype::Aircraft,
    VisualArchetype::Ship,
    VisualArchetype::ConstructionYard,
    VisualArchetype::PowerPlant,
    VisualArchetype::Barracks,
    VisualArchetype::Refinery,
    VisualArchetype::WarFactory,
    VisualArchetype::Turret,
    VisualArchetype::GenericBox,
];

#[allow(clippy::too_many_arguments)]
pub(super) fn show_shared_combat_specs_and_visuals(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + Copy,
    abilities: &mut Vec<AbilityDef>,
    container: &mut ContainerSpec,
    stealth: &mut StealthSensorSpec,
    support_aura: &mut SupportAuraSpec,
    point_defense: &mut PointDefenseSpec,
    death_effect: &mut DeathPayloadSpec,
    visual: &mut VisualDef,
    script_hooks: &mut BTreeMap<String, String>,
    unit_ids: &[String],
    vfx_ids: &[String],
) {
    // Active Abilities (`AbilityDef`)
    egui::CollapsingHeader::new(format!(
        "Active Abilities (`abilities`: {})",
        abilities.len()
    ))
    .id_salt((salt, "abilities"))
    .default_open(!abilities.is_empty())
    .show(ui, |ui| {
        if ui.small_button("+ Add AbilityDef").clicked() {
            let n = abilities.len() + 1;
            abilities.push(AbilityDef {
                id: format!("ability_{n}"),
                name: format!("Active Ability {n}"),
                cooldown_secs: 20.0,
                cost: 0,
                range: 10.0,
                radius: 4.0,
                target_mode: AbilityTargetMode::SelfOnly,
                applies_status: None,
                payloads: Vec::new(),
                script_hook: None,
                ..Default::default()
            });
        }
        let mut rem_ab = None;
        for (idx, ab) in abilities.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut ab.id)
                            .id_salt((salt, "ab_id", idx))
                            .desired_width(100.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut ab.name)
                            .id_salt((salt, "ab_nm", idx))
                            .desired_width(120.0),
                    );
                    egui::ComboBox::from_id_salt((salt, "ab_tm", idx))
                        .selected_text(format!("{:?}", ab.target_mode))
                        .width(105.0)
                        .show_ui(ui, |ui| {
                            for tm in [
                                AbilityTargetMode::SelfOnly,
                                AbilityTargetMode::SingleEntity,
                                AbilityTargetMode::GroundPoint,
                                AbilityTargetMode::AreaRadius,
                            ] {
                                ui.selectable_value(&mut ab.target_mode, tm, format!("{tm:?}"));
                            }
                        });
                    if ui.small_button("✖").clicked() {
                        rem_ab = Some(idx);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut ab.cooldown_secs)
                            .range(0.5..=300.0)
                            .speed(0.5)
                            .prefix("CD: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut ab.cost)
                            .range(0..=10000)
                            .prefix("$"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut ab.range)
                            .range(0.0..=48.0)
                            .prefix("Rng: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut ab.radius)
                            .range(0.0..=24.0)
                            .prefix("Rad: "),
                    );
                });
                edit_optional_ref_dropdown(
                    ui,
                    "Cast VFX (`cast_vfx_id`):",
                    &format!("ab_cvfx_{idx}"),
                    &mut ab.cast_vfx_id,
                    vfx_ids,
                );
                edit_optional_ref_dropdown(
                    ui,
                    "Impact VFX (`impact_vfx_id`):",
                    &format!("ab_ivfx_{idx}"),
                    &mut ab.impact_vfx_id,
                    vfx_ids,
                );
                let mut hook_buf = ab.script_hook.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Script Hook:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut hook_buf)
                                .id_salt((salt, "ab_hk", idx))
                                .desired_width(135.0)
                                .hint_text("on_ability"),
                        )
                        .changed()
                    {
                        ab.script_hook =
                            (!hook_buf.trim().is_empty()).then(|| hook_buf.trim().to_string());
                    }
                    if ui.small_button("+ AreaDamage Payload").clicked() {
                        ab.payloads.push(EffectPayload::AreaDamage {
                            radius: 4.0,
                            damage: 120.0,
                            friendly_fire: false,
                        });
                    }
                });
                let mut has_st = ab.applies_status.is_some();
                if ui
                    .checkbox(&mut has_st, "Applies StatusEffectDef")
                    .changed()
                {
                    ab.applies_status = has_st.then(StatusEffectDef::default);
                }
                if let Some(st) = ab.applies_status.as_mut() {
                    edit_status_effect_compact(ui, (salt, "ab_st", idx), st);
                }
            });
        }
        if let Some(i) = rem_ab {
            abilities.remove(i);
        }
    });

    // Container / Garrison, Stealth & Sensors, Support Aura, Point Defense, Death Payload
    egui::CollapsingHeader::new("Container, Stealth, Support Aura, Point Defense & Death")
        .id_salt((salt, "specs"))
        .default_open(false)
        .show(ui, |ui| {
            ui.strong("Container / Garrison (`ContainerSpec`)");
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut container.capacity)
                        .range(0..=30)
                        .prefix("Cap: "),
                );
                ui.checkbox(&mut container.accepts_vehicles, "Vehicles");
                ui.checkbox(&mut container.passengers_can_fire, "Fire Ports");
                ui.checkbox(&mut container.tunnel_network, "Tunnel Net");
                ui.checkbox(&mut container.adapts_weapon_to_passenger, "IFV Turret");
                ui.add(
                    egui::DragValue::new(&mut container.power_per_occupant)
                        .range(-50..=200)
                        .prefix("MW/Occ: "),
                );
            });

            ui.separator();
            ui.strong("Stealth & Sensors (`StealthSensorSpec`)");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut stealth.cloaked, "Cloaked");
                ui.checkbox(&mut stealth.detects_cloak, "Detects Cloak");
                ui.checkbox(&mut stealth.detects_disguise, "Detects Disguise");
                ui.checkbox(&mut stealth.disguises_as_terrain, "Mirage Disguise");
                ui.checkbox(&mut stealth.can_disguise_as_enemy, "Enemy Disguise");
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut stealth.cloak_field_radius)
                        .range(0.0..=30.0)
                        .speed(0.5)
                        .prefix("CloakRad: "),
                );
                ui.add(
                    egui::DragValue::new(&mut stealth.psychic_sensor_radius)
                        .range(0.0..=40.0)
                        .speed(0.5)
                        .prefix("PsiRad: "),
                );
                ui.add(
                    egui::DragValue::new(&mut stealth.reveal_on_fire_secs)
                        .range(0.0..=15.0)
                        .speed(0.25)
                        .prefix("Uncloak(s): "),
                );
            });

            ui.separator();
            ui.strong("Support Aura & Radar (`SupportAuraSpec`)");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut support_aura.grants_radar, "Grants Tactical Radar");
                let mut rev_sec = support_aura.periodic_reveal_interval_secs.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut rev_sec)
                            .range(0..=600)
                            .prefix("SatSweep(s): "),
                    )
                    .changed()
                {
                    support_aura.periodic_reveal_interval_secs =
                        if rev_sec == 0 { None } else { Some(rev_sec) };
                }
                ui.add(
                    egui::DragValue::new(&mut support_aura.periodic_reveal_radius)
                        .range(0.0..=64.0)
                        .prefix("SweepRad: "),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut support_aura.heals_infantry_per_sec)
                        .range(0.0..=200.0)
                        .prefix("InfHeal/s: "),
                );
                ui.add(
                    egui::DragValue::new(&mut support_aura.repairs_vehicles_per_sec)
                        .range(0.0..=200.0)
                        .prefix("VehRep/s: "),
                );
                ui.add(
                    egui::DragValue::new(&mut support_aura.mobile_heal_per_sec)
                        .range(0.0..=200.0)
                        .prefix("AuraHeal/s: "),
                );
                ui.add(
                    egui::DragValue::new(&mut support_aura.mobile_heal_radius)
                        .range(0.0..=24.0)
                        .prefix("AuraRad: "),
                );
                ui.add(
                    egui::DragValue::new(&mut support_aura.cash_trickle_per_sec)
                        .range(0..=500)
                        .prefix("$/s: "),
                );
            });

            ui.separator();
            ui.strong("Point Defense & Death Payload");
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut point_defense.intercept_radius)
                        .range(0.0..=24.0)
                        .speed(0.5)
                        .prefix("PD Rad: "),
                );
                ui.add(
                    egui::DragValue::new(&mut point_defense.cooldown_secs)
                        .range(0.0..=30.0)
                        .speed(0.25)
                        .prefix("PD CD: "),
                );
                ui.add(
                    egui::DragValue::new(&mut death_effect.explosion_damage)
                        .range(0.0..=5000.0)
                        .prefix("DeathDmg: "),
                );
                ui.add(
                    egui::DragValue::new(&mut death_effect.explosion_radius)
                        .range(0.0..=16.0)
                        .prefix("DeathRad: "),
                );
                ui.checkbox(&mut death_effect.leaves_radiation, "Leaves Rad");
                ui.checkbox(&mut death_effect.drops_salvage, "Drops Salvage");
            });
            ui.horizontal(|ui| {
                ui.label("Survivor Unit:");
                let cur = death_effect.survivor_unit.as_deref().unwrap_or("(None)");
                egui::ComboBox::from_id_salt((salt, "death_surv_cb"))
                    .selected_text(cur)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut death_effect.survivor_unit, None, "(None)");
                        for uid in unit_ids {
                            ui.selectable_value(
                                &mut death_effect.survivor_unit,
                                Some(uid.clone()),
                                uid,
                            );
                        }
                    });
                ui.add(
                    egui::DragValue::new(&mut death_effect.survivor_chance)
                        .range(0.0..=1.0)
                        .speed(0.05)
                        .prefix("Chance: "),
                );
            });
        });

    // VisualDef & 9-Slot AnimClipMap & Script Hooks
    egui::CollapsingHeader::new("3D Visuals, PBR / Team-Mask, 9-Clip AnimClipMap & Script Hooks")
        .id_salt((salt, "visuals"))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("3D Model (.glb):");
                ui.text_edit_singleline(&mut visual.model);
            });
            let mut ico = visual.icon.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Cameo Icon (.png):");
                if ui.text_edit_singleline(&mut ico).changed() {
                    visual.icon = (!ico.trim().is_empty()).then(|| ico.trim().to_string());
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Archetype:");
                egui::ComboBox::from_id_salt((salt, "vis_arch"))
                    .selected_text(format!("{:?}", visual.archetype))
                    .show_ui(ui, |ui| {
                        for a in ALL_VISUAL_ARCHETYPES {
                            ui.selectable_value(&mut visual.archetype, a, format!("{a:?}"));
                        }
                    });
                ui.color_edit_button_rgb(&mut visual.primary_color);
                ui.checkbox(&mut visual.team_color_tint, "Team Tint");
                ui.checkbox(&mut visual.has_turret, "Turret");
                ui.checkbox(&mut visual.has_damage_states, "Damage States");
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Team-Mask:");
                egui::ComboBox::from_id_salt((salt, "vis_tmask"))
                    .selected_text(format!("{:?}", visual.team_mask_mode))
                    .show_ui(ui, |ui| {
                        for m in [
                            TeamMaskMode::AutoNeutralGray,
                            TeamMaskMode::AlphaMask,
                            TeamMaskMode::VertexColorMask,
                            TeamMaskMode::FullTint,
                            TeamMaskMode::Disabled,
                        ] {
                            ui.selectable_value(&mut visual.team_mask_mode, m, format!("{m:?}"));
                        }
                    });
                let mut met = visual.metallic.unwrap_or(0.35);
                if ui
                    .add(
                        egui::DragValue::new(&mut met)
                            .range(0.0..=1.0)
                            .speed(0.02)
                            .prefix("Metal: "),
                    )
                    .changed()
                {
                    visual.metallic = Some(met);
                }
                let mut rgh = visual.roughness.unwrap_or(0.45);
                if ui
                    .add(
                        egui::DragValue::new(&mut rgh)
                            .range(0.04..=1.0)
                            .speed(0.02)
                            .prefix("Rough: "),
                    )
                    .changed()
                {
                    visual.roughness = Some(rgh);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Scale XYZ:");
                ui.add(
                    egui::DragValue::new(&mut visual.scale[0])
                        .range(0.1..=12.0)
                        .speed(0.05),
                );
                ui.add(
                    egui::DragValue::new(&mut visual.scale[1])
                        .range(0.1..=12.0)
                        .speed(0.05),
                );
                ui.add(
                    egui::DragValue::new(&mut visual.scale[2])
                        .range(0.1..=12.0)
                        .speed(0.05),
                );
            });

            ui.separator();
            ui.strong("9-State Skeletal Animation Clip Bindings (`visual.animations`)");
            edit_anim_clip_map(ui, (salt, "anims"), &mut visual.animations);

            ui.separator();
            ui.strong("Entity Script Hooks (`on_spawn`, `on_damaged`, `on_ability`, `on_death`)");
            for hook_key in ["on_spawn", "on_damaged", "on_ability", "on_death"] {
                let mut fn_name = script_hooks.get(hook_key).cloned().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.monospace(format!("{hook_key}:"));
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut fn_name)
                                .id_salt((salt, "shook", hook_key))
                                .desired_width(145.0)
                                .hint_text("script_fn_name"),
                        )
                        .changed()
                    {
                        if fn_name.trim().is_empty() {
                            script_hooks.remove(hook_key);
                        } else {
                            script_hooks.insert(hook_key.to_string(), fn_name.trim().to_string());
                        }
                    }
                });
            }
        });
}

fn edit_anim_clip_map(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + Copy,
    anims: &mut AnimClipMap,
) {
    egui::Grid::new((salt, "anim_grid"))
        .num_columns(4)
        .spacing([6.0, 3.0])
        .show(ui, |ui| {
            let cell = |ui: &mut egui::Ui, key: &str, slot: &mut Option<String>| {
                ui.label(key);
                let mut buf = slot.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut buf)
                            .id_salt((salt, key))
                            .desired_width(95.0)
                            .hint_text("(auto)"),
                    )
                    .changed()
                {
                    *slot = (!buf.trim().is_empty()).then(|| buf.trim().to_string());
                }
            };
            cell(ui, "idle:", &mut anims.idle);
            cell(ui, "walk:", &mut anims.walk);
            ui.end_row();
            cell(ui, "prone:", &mut anims.prone);
            cell(ui, "attack:", &mut anims.attack);
            ui.end_row();
            cell(ui, "deploy:", &mut anims.deploy);
            cell(ui, "die:", &mut anims.die);
            ui.end_row();
            cell(ui, "constructing:", &mut anims.constructing);
            cell(ui, "producing:", &mut anims.producing);
            ui.end_row();
            cell(ui, "damaged:", &mut anims.damaged);
            ui.label("");
            ui.label("");
            ui.end_row();
        });
}

pub(super) fn edit_status_effect_compact(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + Copy,
    st: &mut StatusEffectDef,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut st.id)
                .id_salt((salt, "id"))
                .desired_width(95.0)
                .hint_text("status_id"),
        );
        ui.add(
            egui::DragValue::new(&mut st.duration_secs)
                .range(0.1..=300.0)
                .speed(0.25)
                .prefix("Dur(s): "),
        );
        ui.add(
            egui::DragValue::new(&mut st.dot_dps)
                .range(0.0..=500.0)
                .speed(1.0)
                .prefix("DoT/s: "),
        );
        egui::ComboBox::from_id_salt((salt, "vfx"))
            .selected_text(format!("{:?}", st.vfx_style))
            .width(110.0)
            .show_ui(ui, |ui| {
                for v in [
                    StatusVfxStyle::None,
                    StatusVfxStyle::Ring,
                    StatusVfxStyle::ElectricArcs,
                    StatusVfxStyle::ParasiteSparks,
                    StatusVfxStyle::CountdownBeacon,
                    StatusVfxStyle::TentacleGrapple,
                    StatusVfxStyle::TemporalVortex,
                ] {
                    ui.selectable_value(&mut st.vfx_style, v, format!("{v:?}"));
                }
            });
    });
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::DragValue::new(&mut st.speed_multiplier)
                .range(0.0..=3.0)
                .speed(0.05)
                .prefix("Spd x"),
        );
        ui.add(
            egui::DragValue::new(&mut st.damage_multiplier)
                .range(0.0..=3.0)
                .speed(0.05)
                .prefix("Dmg x"),
        );
        ui.add(
            egui::DragValue::new(&mut st.armor_multiplier)
                .range(0.1..=4.0)
                .speed(0.05)
                .prefix("Arm x"),
        );
        ui.checkbox(&mut st.stunned, "Stun");
        ui.checkbox(&mut st.immobilized, "Root");
        ui.checkbox(&mut st.disarmed, "Disarm");
        ui.checkbox(&mut st.invulnerable, "Invuln");
        ui.checkbox(&mut st.confused, "Confuse");
        ui.checkbox(&mut st.power_charge_boost, "Overcharge");
    });
}
