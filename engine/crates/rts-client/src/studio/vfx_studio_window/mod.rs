mod emitter_inspector;

use crate::rendering::PreviewStageState;
use crate::studio::history::EditorCommand;
use crate::studio::StudioState;
use crate::vfx::VfxPreviewStageState;
use bevy_egui::egui;
use emitter_inspector::show_vfx_effect_and_emitters_inspector;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    ProjectileStyle, VfxBlendMode, VfxDecalLinkSpec, VfxEffectDef, VfxEmitterDef, VfxEmitterShape,
};
use rts_simulation::world::SimWorld;

/// Persistent UI state for the Interactive 3D `VFX & Projectile Studio` (`Task 244`).
#[derive(Debug, Clone)]
pub struct VfxStudioUiState {
    pub selected_effect_id: String,
    pub selected_emitter_idx: usize,
    pub new_effect_id: String,
    pub working_effect: Option<VfxEffectDef>,
    pub pending_showcase_stage_sync: bool,
    pub assign_weapon_id: String,
    pub assign_weapon_slot: usize,
    pub assign_unit_id: String,
    pub assign_building_id: String,
    pub assign_ability_idx: usize,
}

impl Default for VfxStudioUiState {
    fn default() -> Self {
        Self {
            selected_effect_id: String::new(),
            selected_emitter_idx: 0,
            new_effect_id: "plasma_shock_burst".to_string(),
            working_effect: None,
            pending_showcase_stage_sync: false,
            assign_weapon_id: String::new(),
            assign_weapon_slot: 2,
            assign_unit_id: String::new(),
            assign_building_id: String::new(),
            assign_ability_idx: 0,
        }
    }
}

/// Builds a rich multi-emitter HDR `VfxEffectDef` preset with plasma sparks, shockwave ring, and scorch decal.
pub fn build_starter_plasma_vfx_effect(id: &str) -> VfxEffectDef {
    VfxEffectDef {
        id: id.to_string(),
        name: format!("HDR Plasma Burst ({id})"),
        duration_secs: 1.35,
        looping: true,
        camera_shake_intensity: 0.35,
        camera_shake_radius: 18.0,
        ground_decal: Some(VfxDecalLinkSpec {
            decal_kind: "scorch_blast".to_string(),
            radius: 2.6,
            fade_time_secs: 14.0,
        }),
        fallback_style: ProjectileStyle::Tesla,
        emitters: vec![
            VfxEmitterDef {
                id: "core_sparks".to_string(),
                shape: VfxEmitterShape::Cone,
                blend_mode: VfxBlendMode::Additive,
                burst_count: 48,
                continuous_rate_per_sec: 55.0,
                duration_secs: 1.1,
                looping: true,
                lifetime_range_secs: [0.55, 1.15],
                speed_range: [3.2, 8.5],
                cone_angle_deg: 42.0,
                sphere_radius: 0.35,
                gravity: -6.5,
                drag: 1.4,
                size_ramp: [0.35, 0.72, 0.16],
                color_ramp_rgba: [
                    [2.4, 2.8, 3.5, 1.0],
                    [0.35, 1.65, 2.85, 0.82],
                    [0.08, 0.25, 0.95, 0.0],
                ],
                beam_width: 0.25,
                ribbon_width: 0.20,
                texture_path: None,
                local_offset: [0.0, 0.65, 0.0],
            },
            VfxEmitterDef {
                id: "shock_ring".to_string(),
                shape: VfxEmitterShape::Ring,
                blend_mode: VfxBlendMode::Additive,
                burst_count: 24,
                continuous_rate_per_sec: 16.0,
                duration_secs: 0.65,
                looping: true,
                lifetime_range_secs: [0.40, 0.75],
                speed_range: [4.5, 6.2],
                cone_angle_deg: 15.0,
                sphere_radius: 0.85,
                gravity: 0.0,
                drag: 2.2,
                size_ramp: [0.36, 0.78, 0.16],
                color_ramp_rgba: [
                    [2.8, 1.95, 0.65, 0.95],
                    [1.65, 0.65, 0.20, 0.55],
                    [0.25, 0.08, 0.04, 0.0],
                ],
                beam_width: 0.20,
                ribbon_width: 0.18,
                texture_path: None,
                local_offset: [0.0, 0.15, 0.0],
            },
        ],
    }
}

pub fn show_vfx_studio_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    preview_stage: Option<&mut PreviewStageState>,
    mut vfx_stage: Option<&mut VfxPreviewStageState>,
) {
    // Ensure a valid selected VFX effect when catalog is non-empty
    if (state.vfx_studio_ui.selected_effect_id.is_empty()
        || !loaded
            .vfx_effects
            .contains_key(&state.vfx_studio_ui.selected_effect_id))
        && !loaded.vfx_effects.is_empty()
    {
        if let Some(first_id) = loaded.vfx_effects.keys().next() {
            state.vfx_studio_ui.selected_effect_id = first_id.clone();
        }
    }

    // Top Catalog Bar (`+ New VFX`, Select, Delete, Save `vfx/vfx.toml`)
    ui.horizontal_wrapped(|ui| {
        ui.strong("VFX Catalog (`vfx/vfx.toml`):");
        let effect_ids: Vec<String> = loaded.vfx_effects.keys().cloned().collect();
        let cur_sel = if state.vfx_studio_ui.selected_effect_id.is_empty() {
            "(None)"
        } else {
            state.vfx_studio_ui.selected_effect_id.as_str()
        };
        egui::ComboBox::from_id_salt("vfx_studio_effect_pick")
            .selected_text(cur_sel)
            .width(165.0)
            .show_ui(ui, |ui| {
                for eid in &effect_ids {
                    if ui
                        .selectable_value(
                            &mut state.vfx_studio_ui.selected_effect_id,
                            eid.clone(),
                            eid,
                        )
                        .clicked()
                    {
                        state.vfx_studio_ui.selected_emitter_idx = 0;
                    }
                }
            });

        ui.add(
            egui::TextEdit::singleline(&mut state.vfx_studio_ui.new_effect_id)
                .desired_width(120.0)
                .hint_text("new_vfx_id"),
        );
        if ui
            .button(
                egui::RichText::new("✨ + Create Preset VFX")
                    .color(egui::Color32::from_rgb(115, 240, 185)),
            )
            .clicked()
        {
            let clean_id = sanitize_vfx_id(&state.vfx_studio_ui.new_effect_id);
            if !clean_id.is_empty() {
                let before = loaded.vfx_effects.get(&clean_id).cloned().map(Box::new);
                let created = build_starter_plasma_vfx_effect(&clean_id);
                loaded.vfx_effects.insert(clean_id.clone(), created.clone());
                state.vfx_studio_ui.selected_effect_id = clean_id.clone();
                state.vfx_studio_ui.selected_emitter_idx = 0;
                state
                    .history
                    .push_executed(EditorCommand::EditVfxEffectDef {
                        id: clean_id.clone(),
                        before,
                        after: Some(Box::new(created)),
                    });
                if let Some(vs) = vfx_stage.as_deref_mut() {
                    vs.trigger_burst = true;
                }
                state.status_banner = format!("✔ Created multi-emitter VFX '{clean_id}'.");
            }
        }

        if ui.button("💾 Save vfx/vfx.toml").clicked() {
            match loaded.save_vfx_toml_catalog() {
                Ok(p) => {
                    state.status_banner = format!(
                        "✔ Saved {} VFX definition(s) -> {}",
                        loaded.vfx_effects.len(),
                        p.display()
                    );
                }
                Err(e) => state.status_banner = format!("✖ Save vfx.toml error: {e}"),
            }
        }
    });

    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("studio_vfx_studio_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let sel_id = state.vfx_studio_ui.selected_effect_id.clone();
            let Some(before_eff) = loaded.vfx_effects.get(&sel_id).cloned() else {
                ui.weak(
                    "No VfxEffectDef in active mod yet. Click [✨ + Create Preset VFX] above to author multi-emitter particles, beams & ground decals.",
                );
                return;
            };
            let mut draft = before_eff.clone();

            // 1. Live Isolated 3D Preview Stage + Timeline Scrubber + [💥 Trigger Burst]
            if state.vfx_studio_ui.pending_showcase_stage_sync {
                state.vfx_studio_ui.pending_showcase_stage_sync = false;
                if let Some(vs) = vfx_stage.as_deref_mut() {
                    vs.active_effect_id = Some(sel_id.clone());
                    vs.override_effect = Some(draft.clone());
                    vs.trigger_burst = true;
                }
            }
            show_live_3d_vfx_stage_panel(
                ui,
                state,
                &draft,
                preview_stage,
                vfx_stage.as_deref_mut(),
            );

            ui.separator();

            // 2. Effect & Multi-Emitter Curve/Ramp Editor
            let mut delete_effect = false;
            show_vfx_effect_and_emitters_inspector(
                ui,
                &mut draft,
                &mut state.vfx_studio_ui.selected_emitter_idx,
                &mut delete_effect,
            );

            if delete_effect {
                let _ = loaded.delete_vfx_effect_def(&sel_id);
                state.vfx_studio_ui.selected_effect_id = loaded
                    .vfx_effects
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_default();
                state.history.push_executed(EditorCommand::EditVfxEffectDef {
                    id: sel_id,
                    before: Some(Box::new(before_eff)),
                    after: None,
                });
                return;
            }

            if draft != before_eff {
                loaded.vfx_effects.insert(sel_id.clone(), draft.clone());
                if let Some(vs) = vfx_stage {
                    vs.active_effect_id = Some(sel_id.clone());
                    vs.override_effect = Some(draft.clone());
                }
                state.validation_dirty = true;
                state.history.push_executed(EditorCommand::EditVfxEffectDef {
                    id: sel_id.clone(),
                    before: Some(Box::new(before_eff)),
                    after: Some(Box::new(draft)),
                });
            }

            ui.separator();

            // 3. 1-Click Assignment to WeaponDef, UnitDef/AbilityDef, or BuildingDef
            show_one_click_vfx_assignment_bar(ui, &sel_id, state, loaded, world);
        });
}

fn show_live_3d_vfx_stage_panel(
    ui: &mut egui::Ui,
    state: &StudioState,
    effect: &VfxEffectDef,
    preview_stage: Option<&mut PreviewStageState>,
    vfx_stage: Option<&mut VfxPreviewStageState>,
) {
    if let Some(vs) = vfx_stage {
        vs.active_effect_id = Some(effect.id.clone());
        vs.override_effect = Some(effect.clone());

        ui.horizontal_wrapped(|ui| {
            if ui
                .button(
                    egui::RichText::new("💥 Trigger Burst")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 190, 80)),
                )
                .clicked()
            {
                vs.trigger_burst = true;
                vs.timeline_secs = 0.0;
                vs.playback_paused = false;
            }
            ui.checkbox(&mut vs.loop_enabled, "Loop");
            ui.checkbox(&mut vs.playback_paused, "Pause");
            let max_t = effect.duration_secs.max(0.2);
            ui.add(
                egui::Slider::new(&mut vs.timeline_secs, 0.0..=max_t)
                    .text("Timeline (s)")
                    .fixed_decimals(2),
            );
        });
    }

    let avail_w = ui.available_width().clamp(210.0, 420.0);
    let view_sz = egui::vec2(avail_w, (avail_w * 0.62).round());
    if let Some((_, tex_id)) = state.preview_stage_egui_tex {
        let (rect, resp) = ui.allocate_exact_size(view_sz, egui::Sense::click_and_drag());
        ui.painter().image(
            tex_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(1.4, egui::Color32::from_rgb(80, 185, 245)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 8.0, rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            format!(
                "3D VFX Stage: {} ({} emitter[s]) — Drag to Orbit, Scroll to Zoom",
                effect.name,
                effect.emitters.len()
            ),
            egui::FontId::proportional(10.5),
            egui::Color32::from_rgb(210, 238, 255),
        );
        if let Some(st) = preview_stage {
            if resp.dragged() {
                let d = resp.drag_delta();
                st.auto_turntable = false;
                st.orbit_yaw += d.x * 0.012;
                st.orbit_pitch = (st.orbit_pitch + d.y * 0.010).clamp(0.08, 1.42);
            }
            if resp.hovered() {
                let sy = ui.input(|i| i.smooth_scroll_delta.y);
                if sy.abs() > 0.01 {
                    st.orbit_distance = (st.orbit_distance - sy * 0.015).clamp(1.8, 24.0);
                }
            }
        }
    } else {
        ui.weak("3D Offscreen PreviewStage texture initializing...");
    }
}

fn show_one_click_vfx_assignment_bar(
    ui: &mut egui::Ui,
    vfx_id: &str,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    ui.strong("🔗 1-Click Bind VFX to Weapon / Unit / Ability / Building");
    let weapon_ids: Vec<String> = loaded.weapons.keys().cloned().collect();
    if state.vfx_studio_ui.assign_weapon_id.is_empty() {
        if let Some(w) = weapon_ids.first() {
            state.vfx_studio_ui.assign_weapon_id = w.clone();
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Weapon:");
        egui::ComboBox::from_id_salt("vfx_bind_wpn")
            .selected_text(&state.vfx_studio_ui.assign_weapon_id)
            .width(125.0)
            .show_ui(ui, |ui| {
                for wid in &weapon_ids {
                    ui.selectable_value(
                        &mut state.vfx_studio_ui.assign_weapon_id,
                        wid.clone(),
                        wid,
                    );
                }
            });
        for (slot_i, lbl) in [(0usize, "Muzzle"), (1, "Projectile"), (2, "Impact")] {
            ui.selectable_value(&mut state.vfx_studio_ui.assign_weapon_slot, slot_i, lbl);
        }
        if ui.small_button("⚡ Bind to Weapon & Save").clicked() {
            let wid = state.vfx_studio_ui.assign_weapon_id.clone();
            if let Some(before) = loaded.weapons.get(&wid).cloned() {
                let mut after = before.clone();
                match state.vfx_studio_ui.assign_weapon_slot {
                    0 => after.muzzle_vfx_id = Some(vfx_id.to_string()),
                    1 => after.projectile_vfx_id = Some(vfx_id.to_string()),
                    _ => after.impact_vfx_id = Some(vfx_id.to_string()),
                }
                let _ = loaded.save_weapon_def_toml(after.clone());
                let _ = loaded.save_vfx_toml_catalog();
                let _ = world.apply_mod_hot_reload(loaded);
                state.history.push_executed(EditorCommand::EditWeaponDef {
                    id: wid.clone(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
                state.status_banner = format!("✔ Bound VFX '{vfx_id}' onto Weapon '{wid}'.");
            }
        }
    });

    let unit_ids: Vec<String> = loaded.units.keys().cloned().collect();
    if state.vfx_studio_ui.assign_unit_id.is_empty() {
        if let Some(u) = unit_ids.first() {
            state.vfx_studio_ui.assign_unit_id = u.clone();
        }
    }
    let building_ids: Vec<String> = loaded.buildings.keys().cloned().collect();
    if state.vfx_studio_ui.assign_building_id.is_empty() {
        if let Some(b) = building_ids.first() {
            state.vfx_studio_ui.assign_building_id = b.clone();
        }
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("Unit:");
        egui::ComboBox::from_id_salt("vfx_bind_unit")
            .selected_text(&state.vfx_studio_ui.assign_unit_id)
            .width(110.0)
            .show_ui(ui, |ui| {
                for uid in &unit_ids {
                    ui.selectable_value(&mut state.vfx_studio_ui.assign_unit_id, uid.clone(), uid);
                }
            });
        if ui.small_button("Bind Unit Death VFX").clicked() {
            let uid = state.vfx_studio_ui.assign_unit_id.clone();
            if let Some(before) = loaded.units.get(&uid).cloned() {
                let mut after = before.clone();
                after.death_vfx_id = Some(vfx_id.to_string());
                let _ = loaded.save_unit_def_toml(after.clone());
                let _ = world.apply_mod_hot_reload(loaded);
                state.history.push_executed(EditorCommand::EditUnitDef {
                    id: uid.clone(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
                state.status_banner = format!("✔ Bound death_vfx_id='{vfx_id}' on Unit '{uid}'.");
            }
        }
        if ui.small_button("Bind Ability#1 Impact").clicked() {
            let uid = state.vfx_studio_ui.assign_unit_id.clone();
            if let Some(before) = loaded.units.get(&uid).cloned() {
                let mut after = before.clone();
                if let Some(ab) = after.abilities.first_mut() {
                    ab.impact_vfx_id = Some(vfx_id.to_string());
                    let _ = loaded.save_unit_def_toml(after.clone());
                    state.history.push_executed(EditorCommand::EditUnitDef {
                        id: uid.clone(),
                        before: Box::new(before),
                        after: Box::new(after),
                    });
                    state.status_banner =
                        format!("✔ Bound impact_vfx_id='{vfx_id}' on '{uid}' Ability #1.");
                }
            }
        }

        ui.label("Bldg:");
        egui::ComboBox::from_id_collapse_button_fix(
            "vfx_bind_bldg",
            ui,
            &building_ids,
            &mut state.vfx_studio_ui.assign_building_id,
        );
        if ui.small_button("Bind Bldg Death VFX").clicked() {
            let bid = state.vfx_studio_ui.assign_building_id.clone();
            if let Some(before) = loaded.buildings.get(&bid).cloned() {
                let mut after = before.clone();
                after.death_vfx_id = Some(vfx_id.to_string());
                let _ = loaded.save_building_def_toml(after.clone());
                let _ = world.apply_mod_hot_reload(loaded);
                state.history.push_executed(EditorCommand::EditBuildingDef {
                    id: bid.clone(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
                state.status_banner =
                    format!("✔ Bound death_vfx_id='{vfx_id}' on Building '{bid}'.");
            }
        }
    });
}

trait ComboHelper {
    fn from_id_collapse_button_fix(
        salt: &str,
        ui: &mut egui::Ui,
        ids: &[String],
        current: &mut String,
    );
}

impl ComboHelper for egui::ComboBox {
    fn from_id_collapse_button_fix(
        salt: &str,
        ui: &mut egui::Ui,
        ids: &[String],
        current: &mut String,
    ) {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(current.as_str())
            .width(110.0)
            .show_ui(ui, |ui| {
                for id in ids {
                    ui.selectable_value(current, id.clone(), id);
                }
            });
    }
}

fn sanitize_vfx_id(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfx_catalog_preset_roundtrips_toml_and_binds_to_weapon() {
        let temp_dir =
            std::env::temp_dir().join(format!("rust_rts_vfx_studio_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        rts_asset_pipeline::scaffold::scaffold_new_mod_project(
            &temp_dir,
            "vfx-studio-test",
            "VFX Studio Test",
            None,
        )
        .expect("scaffold mod");

        let mut loaded = LoadedMod::load_from_dir(&temp_dir).expect("load mod");
        let eff = build_starter_plasma_vfx_effect("ion_burst_mk2");
        let saved = loaded
            .upsert_vfx_effect_def(eff.clone())
            .expect("save vfx/vfx.toml");
        assert!(saved.exists());

        let wid = loaded.weapons.keys().next().expect("weapon").clone();
        let mut wpn = loaded.weapons[&wid].clone();
        wpn.impact_vfx_id = Some("ion_burst_mk2".to_string());
        loaded
            .save_weapon_def_toml(wpn)
            .expect("save weapon with impact_vfx_id");

        let reloaded = LoadedMod::load_from_dir(&temp_dir).expect("reload mod");
        assert_eq!(reloaded.vfx_effects.get("ion_burst_mk2"), Some(&eff));
        assert_eq!(
            reloaded.weapons[&wid].impact_vfx_id.as_deref(),
            Some("ion_burst_mk2")
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
