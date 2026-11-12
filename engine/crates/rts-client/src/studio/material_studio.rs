use super::history::EditorCommand;
use super::{SdkInspectTarget, StudioState};
use crate::rendering::{MaterialPreviewOverrideResource, PreviewStageState};
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{TeamMaskMode, VisualDef};
use rts_simulation::world::SimWorld;

/// 8-slot RTS Team/Faction Swatch Palette for live 3D Turntable PBR & Team-Mask preview (`Task 245`).
pub const TEAM_SLOT_SWATCHES_RGB: [([f32; 3], &str); 8] = [
    ([0.18, 0.48, 0.96], "Slot 1 Cobalt"),
    ([0.92, 0.18, 0.18], "Slot 2 Crimson"),
    ([0.66, 0.24, 0.88], "Slot 3 Psi Violet"),
    ([0.96, 0.78, 0.18], "Slot 4 Sovereign Gold"),
    ([0.18, 0.82, 0.42], "Slot 5 Emerald"),
    ([0.16, 0.84, 0.92], "Slot 6 Glacial Cyan"),
    ([0.96, 0.48, 0.14], "Slot 7 Amber"),
    ([0.92, 0.26, 0.68], "Slot 8 Magenta"),
];

/// Persistent UI state for the Interactive PBR Material & Team-Mask Studio (`Task 245`).
#[derive(Debug, Clone)]
pub struct MaterialStudioUiState {
    pub override_enabled: bool,
    pub preview_team_slot: u8,
}

impl Default for MaterialStudioUiState {
    fn default() -> Self {
        Self {
            override_enabled: true,
            preview_team_slot: 1,
        }
    }
}

/// Renders the Interactive PBR Material & Team-Mask Studio inside Subwindow 7 (`AssetPreviewer`).
pub fn show_pbr_material_and_team_mask_studio(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    mut preview_stage: Option<&mut PreviewStageState>,
    mut mat_override: Option<&mut MaterialPreviewOverrideResource>,
) {
    // Ensure a Unit or Building is selected for PBR editing if None is active
    if !matches!(
        state.inspect_target,
        SdkInspectTarget::Unit(_) | SdkInspectTarget::Building(_)
    ) {
        if let Some(first_uid) = loaded.units.keys().next().cloned() {
            state.inspect_target = SdkInspectTarget::Unit(first_uid);
        } else if let Some(first_bid) = loaded.buildings.keys().next().cloned() {
            state.inspect_target = SdkInspectTarget::Building(first_bid);
        }
    }

    // Quick Entity Target Selector + Live Override Toggle
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(
            &mut state.material_studio_ui.override_enabled,
            "Live 3D Stage PBR Override",
        );
        ui.label("Target:");
        let cur_lbl = match &state.inspect_target {
            SdkInspectTarget::Unit(u) => format!("Unit: {u}"),
            SdkInspectTarget::Building(b) => format!("Building: {b}"),
            _ => "(Select Unit/Building)".to_string(),
        };
        egui::ComboBox::from_id_salt("pbr_studio_target_pick")
            .selected_text(cur_lbl)
            .width(185.0)
            .show_ui(ui, |ui| {
                ui.strong("Units");
                for uid in loaded.units.keys() {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Unit(uid.clone()),
                            uid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Unit(uid.clone());
                    }
                }
                ui.separator();
                ui.strong("Buildings");
                for bid in loaded.buildings.keys() {
                    if ui
                        .selectable_label(
                            state.inspect_target == SdkInspectTarget::Building(bid.clone()),
                            bid,
                        )
                        .clicked()
                    {
                        state.inspect_target = SdkInspectTarget::Building(bid.clone());
                    }
                }
            });
    });

    // Live 8-Slot Team-Color Swatch Bar (`Slot 1..8`)
    ui.horizontal_wrapped(|ui| {
        ui.strong("Team-Color Swatch (1..8):");
        for (idx, (rgb, name)) in TEAM_SLOT_SWATCHES_RGB.iter().enumerate() {
            let slot = (idx + 1) as u8;
            let active = state.material_studio_ui.preview_team_slot == slot;
            let fill = egui::Color32::from_rgb(
                (rgb[0] * 255.0) as u8,
                (rgb[1] * 255.0) as u8,
                (rgb[2] * 255.0) as u8,
            );
            let btn = egui::Button::new(
                egui::RichText::new(format!(" {slot} "))
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(fill)
            .stroke(if active {
                egui::Stroke::new(2.2, egui::Color32::from_rgb(255, 245, 150))
            } else {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 42, 54))
            });
            if ui.add(btn).on_hover_text(*name).clicked() {
                state.material_studio_ui.preview_team_slot = slot;
                if let Some(st) = preview_stage.as_deref_mut() {
                    st.faction_tint_rgb = *rgb;
                }
            }
        }
    });

    match state.inspect_target.clone() {
        SdkInspectTarget::Unit(uid) => {
            let Some(before) = loaded.units.get(&uid).cloned() else {
                return;
            };
            let mut draft = before.clone();
            let save_clicked = edit_visual_pbr_controls(ui, &mut draft.visual);

            sync_material_override_resource(
                &state.material_studio_ui,
                &draft.visual,
                mat_override.as_deref_mut(),
            );

            if save_clicked {
                match loaded.save_unit_def_toml(draft.clone()) {
                    Ok(p) => {
                        state.modified_units.remove(&uid);
                        let _ = world.apply_mod_hot_reload(loaded);
                        state.status_banner =
                            format!("✔ Saved PBR Material for Unit '{uid}' -> {}", p.display());
                    }
                    Err(e) => state.status_banner = format!("✖ Save error: {e}"),
                }
            }
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
        SdkInspectTarget::Building(bid) => {
            let Some(before) = loaded.buildings.get(&bid).cloned() else {
                return;
            };
            let mut draft = before.clone();
            let save_clicked = edit_visual_pbr_controls(ui, &mut draft.visual);

            sync_material_override_resource(
                &state.material_studio_ui,
                &draft.visual,
                mat_override.as_deref_mut(),
            );

            if save_clicked {
                match loaded.save_building_def_toml(draft.clone()) {
                    Ok(p) => {
                        state.modified_buildings.remove(&bid);
                        let _ = world.apply_mod_hot_reload(loaded);
                        state.status_banner = format!(
                            "✔ Saved PBR Material for Building '{bid}' -> {}",
                            p.display()
                        );
                    }
                    Err(e) => state.status_banner = format!("✖ Save error: {e}"),
                }
            }
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
        _ => {
            if let Some(mo) = mat_override {
                mo.enabled = false;
            }
        }
    }
}

fn sync_material_override_resource(
    ui_state: &MaterialStudioUiState,
    visual: &VisualDef,
    mat_override: Option<&mut MaterialPreviewOverrideResource>,
) {
    if let Some(mo) = mat_override {
        mo.enabled = ui_state.override_enabled;
        mo.preview_team_slot = ui_state.preview_team_slot.clamp(1, 8);
        mo.visual_override = ui_state.override_enabled.then(|| visual.clone());
    }
}

fn edit_visual_pbr_controls(ui: &mut egui::Ui, visual: &mut VisualDef) -> bool {
    let mut save_clicked = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Primary Albedo:");
        ui.color_edit_button_rgb(&mut visual.primary_color);
        ui.checkbox(&mut visual.team_color_tint, "Team Color Tint");
        ui.label("Mask Mode:");
        egui::ComboBox::from_id_salt("pbr_team_mask_mode_cb")
            .selected_text(format!("{:?}", visual.team_mask_mode))
            .width(145.0)
            .show_ui(ui, |ui| {
                for mode in [
                    TeamMaskMode::AutoNeutralGray,
                    TeamMaskMode::AlphaMask,
                    TeamMaskMode::VertexColorMask,
                    TeamMaskMode::FullTint,
                    TeamMaskMode::Disabled,
                ] {
                    ui.selectable_value(&mut visual.team_mask_mode, mode, format!("{mode:?}"));
                }
            });
        if ui
            .button(
                egui::RichText::new("💾 Save PBR Material (.toml)")
                    .color(egui::Color32::from_rgb(115, 235, 175)),
            )
            .clicked()
        {
            save_clicked = true;
        }
    });

    // Metallic / Roughness / Emissive Boost / Mask Threshold Sliders
    let mut metallic = visual.metallic.unwrap_or(0.35);
    if ui
        .add(
            egui::Slider::new(&mut metallic, 0.0..=1.0)
                .text("Metallic (`metallic`)")
                .fixed_decimals(2),
        )
        .changed()
    {
        visual.metallic = Some(metallic);
    }

    let mut roughness = visual.roughness.unwrap_or(0.45);
    if ui
        .add(
            egui::Slider::new(&mut roughness, 0.04..=1.0)
                .text("Roughness (`roughness`)")
                .fixed_decimals(2),
        )
        .changed()
    {
        visual.roughness = Some(roughness);
    }

    let mut emissive = visual.emissive_boost.unwrap_or(0.0);
    if ui
        .add(
            egui::Slider::new(&mut emissive, 0.0..=8.0)
                .text("HDR Emissive Boost (`emissive_boost`)")
                .fixed_decimals(2),
        )
        .changed()
    {
        visual.emissive_boost = Some(emissive);
    }

    let mut mask_thresh = visual.team_mask_threshold.unwrap_or(0.14);
    if ui
        .add(
            egui::Slider::new(&mut mask_thresh, 0.01..=0.60)
                .text("Neutral-Gray Chroma Threshold (`team_mask_threshold`)")
                .fixed_decimals(2),
        )
        .changed()
    {
        visual.team_mask_threshold = Some(mask_thresh);
    }

    let mut norm_buf = visual.normal_map_path.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Normal Map (.png):");
        if ui
            .add(
                egui::TextEdit::singleline(&mut norm_buf)
                    .desired_width(210.0)
                    .hint_text("textures/pbr/unit_normal.png"),
            )
            .changed()
        {
            visual.normal_map_path =
                (!norm_buf.trim().is_empty()).then(|| norm_buf.trim().to_string());
        }
    });

    let mut orm_buf = visual.orm_map_path.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("ORM Map (Occ/Rough/Metal):");
        if ui
            .add(
                egui::TextEdit::singleline(&mut orm_buf)
                    .desired_width(210.0)
                    .hint_text("textures/pbr/unit_orm.png"),
            )
            .changed()
        {
            visual.orm_map_path = (!orm_buf.trim().is_empty()).then(|| orm_buf.trim().to_string());
        }
    });

    save_clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbr_material_and_team_mask_fields_roundtrip_unit_toml() {
        let temp_dir =
            std::env::temp_dir().join(format!("rust_rts_pbr_studio_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        rts_asset_pipeline::scaffold::scaffold_new_mod_project(
            &temp_dir,
            "pbr-studio-test",
            "PBR Studio Test",
            None,
        )
        .expect("scaffold mod");

        let mut loaded = LoadedMod::load_from_dir(&temp_dir).expect("load mod");
        let uid = loaded.units.keys().next().expect("unit").clone();
        let mut unit = loaded.units[&uid].clone();
        unit.visual.metallic = Some(0.82);
        unit.visual.roughness = Some(0.24);
        unit.visual.emissive_boost = Some(2.75);
        unit.visual.team_mask_mode = TeamMaskMode::VertexColorMask;
        unit.visual.team_mask_threshold = Some(0.19);
        unit.visual.normal_map_path = Some("textures/pbr/hull_n.png".to_string());
        unit.visual.orm_map_path = Some("textures/pbr/hull_orm.png".to_string());

        loaded
            .save_unit_def_toml(unit)
            .expect("save unit pbr fields");

        let reloaded = LoadedMod::load_from_dir(&temp_dir).expect("reload mod");
        let v = &reloaded.units[&uid].visual;
        assert_eq!(v.metallic, Some(0.82));
        assert_eq!(v.roughness, Some(0.24));
        assert_eq!(v.emissive_boost, Some(2.75));
        assert_eq!(v.team_mask_mode, TeamMaskMode::VertexColorMask);
        assert_eq!(v.team_mask_threshold, Some(0.19));
        assert_eq!(
            v.normal_map_path.as_deref(),
            Some("textures/pbr/hull_n.png")
        );
        assert_eq!(v.orm_map_path.as_deref(), Some("textures/pbr/hull_orm.png"));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
