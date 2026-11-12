mod importer_and_av;

use super::history::EditorCommand;
use super::material_studio::show_pbr_material_and_team_mask_studio;
use super::{SdkInspectTarget, StudioState};
use crate::audio::SoundSettingsResource;
use crate::rendering::{
    MaterialPreviewOverrideResource, PreviewStageState, StudioDebugOverlayConfig,
    VideoPlaybackResource,
};
use bevy_egui::egui;
use importer_and_av::{
    show_audio_auditioner, show_modern_asset_importer_panel, show_video_auditioner,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{SocketOffset, VisualDef};
use rts_simulation::world::SimWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewAnimState {
    #[default]
    Idle,
    Walk,
    Prone,
    Attack,
    Deploy,
    Die,
    Constructing,
    Producing,
    Damaged,
}

impl PreviewAnimState {
    pub const ALL: [PreviewAnimState; 9] = [
        Self::Idle,
        Self::Walk,
        Self::Prone,
        Self::Attack,
        Self::Deploy,
        Self::Die,
        Self::Constructing,
        Self::Producing,
        Self::Damaged,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Walk => "Walk",
            Self::Prone => "Prone",
            Self::Attack => "Attack",
            Self::Deploy => "Deploy",
            Self::Die => "Die",
            Self::Constructing => "Constructing",
            Self::Producing => "Producing",
            Self::Damaged => "Damaged",
        }
    }

    pub fn stage_clip_key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Prone => "prone",
            Self::Attack => "attack",
            Self::Deploy => "deploy",
            Self::Die => "die",
            Self::Constructing => "constructing",
            Self::Producing => "producing",
            Self::Damaged => "damaged",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn show_asset_previewer_window(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    sound_res: &mut SoundSettingsResource,
    video_res: Option<&mut VideoPlaybackResource>,
    mut preview_stage: Option<&mut PreviewStageState>,
    mat_override: Option<&mut MaterialPreviewOverrideResource>,
    debug_overlay: &mut StudioDebugOverlayConfig,
) {
    egui::ScrollArea::vertical()
        .id_salt("studio_asset_previewer_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 0. Modern Asset Import Wizard (.glb / .png / .ogg / .flac / .wav / .mp4)
            let import_header = egui::CollapsingHeader::new(
                "📥 Modern Asset Importer (.glb / .png / .ogg / .flac / .wav / .mp4)",
            )
            .default_open(false)
            .open(state.modern_import_open.then_some(true));
            import_header.show(ui, |ui| {
                state.modern_import_open = false;
                show_modern_asset_importer_panel(ui, state, loaded, world);
            });

            ui.separator();

            // 1. Isolated Offscreen 3D Turntable Stage (`RenderLayers::layer(3)`) + Interactive PBR Studio + Socket Rigging
            egui::CollapsingHeader::new(
                "1. Offscreen 3D Turntable Stage, PBR Material Studio & Socket Rigging",
            )
            .default_open(true)
            .show(ui, |ui| {
                show_offscreen_turntable_viewport(
                    ui,
                    state,
                    loaded,
                    world,
                    preview_stage.as_deref_mut(),
                );
                ui.separator();

                // 1b. Interactive PBR Material & Team-Mask Studio (Task 245 — visible above fold!)
                egui::CollapsingHeader::new(
                    "1b. 🎨 Interactive PBR Material & Team-Mask Studio (Slots 1..8)",
                )
                .default_open(true)
                .show(ui, |ui| {
                    show_pbr_material_and_team_mask_studio(
                        ui,
                        state,
                        loaded,
                        world,
                        preview_stage.as_deref_mut(),
                        mat_override,
                    );
                });

                ui.separator();
                ui.checkbox(
                    &mut debug_overlay.show_sockets,
                    "Also Render 3D Socket Gizmos in Main Battlefield Viewport (`show_sockets`)",
                );
                show_socket_and_anim_inspector(ui, state, loaded, world, preview_stage);
            });

            ui.separator();

            // 2. 5-Bus Audio & SFX/EVA Auditioner
            egui::CollapsingHeader::new("2. 5-Bus Audio Mixer & SFX / EVA Auditioner")
                .default_open(true)
                .show(ui, |ui| {
                    show_audio_auditioner(ui, state, loaded, sound_res);
                });

            ui.separator();

            // 3. Native In-Engine FMV Video Auditioner (`VideoPlaybackResource`)
            egui::CollapsingHeader::new("3. In-Engine FMV Video Decoder & Texture Preview")
                .default_open(true)
                .show(ui, |ui| {
                    show_video_auditioner(ui, state, loaded, video_res);
                });
        });
}

fn show_offscreen_turntable_viewport(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    preview_stage: Option<&mut PreviewStageState>,
) {
    let Some(stage) = preview_stage else {
        ui.weak("Offscreen 3D PreviewStageState unavailable in headless mode.");
        return;
    };

    // Keep offscreen stage model & sockets synchronized with the active inspected SDK definition
    let inspected_entity_id: Option<(String, bool)> = match &state.inspect_target {
        SdkInspectTarget::Unit(uid) => {
            if let Some(u) = loaded.units.get(uid) {
                stage.inspected_model_path = Some(u.visual.model.clone());
                stage.sockets = u.visual.sockets.clone();
                Some((uid.clone(), false))
            } else {
                None
            }
        }
        SdkInspectTarget::Building(bid) => {
            if let Some(b) = loaded.buildings.get(bid) {
                stage.inspected_model_path = Some(b.visual.model.clone());
                stage.sockets = b.visual.sockets.clone();
                Some((bid.clone(), true))
            } else {
                None
            }
        }
        _ => None,
    };

    // Top Turntable Control Bar + [📸 Capture Cameo PNG]
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut stage.auto_turntable, "Auto-Turntable");
        ui.checkbox(&mut stage.show_sockets, "3D Socket Crosses");
        if ui.small_button("↺ Reset Camera").clicked() {
            stage.reset_orbit();
        }
        ui.label("Faction Tint:");
        ui.color_edit_button_rgb(&mut stage.faction_tint_rgb);
        if ui
            .button(
                egui::RichText::new("🔥 Test Fire")
                    .color(egui::Color32::from_rgb(255, 185, 80)),
            )
            .on_hover_text("Fires muzzle flash + tracer bolt from Muzzle sockets on isolated Layer 3")
            .clicked()
        {
            stage.trigger_test_fire = true;
        }

        if let Some((ref target_id, is_building)) = inspected_entity_id {
            if ui
                .button(
                    egui::RichText::new("📸 Capture Cameo PNG")
                        .color(egui::Color32::from_rgb(110, 235, 185)),
                )
                .on_hover_text("Captures the current 3D turntable camera angle & faction tint to assets/textures/cameos/<id>.png and binds visual.icon")
                .clicked()
            {
                let rel_icon = format!("assets/textures/cameos/{target_id}.png");
                let out_path = loaded.root_dir.join(&rel_icon);
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                stage.capture_cameo_request = Some((target_id.clone(), out_path.clone()));
                state.cameo_egui_textures.remove(target_id);

                if is_building {
                    if let Some(before) = loaded.buildings.get(target_id).cloned() {
                        let mut after = before.clone();
                        after.visual.icon = Some(rel_icon.clone());
                        loaded.buildings.insert(target_id.clone(), after.clone());
                        state.modified_buildings.insert(target_id.clone());
                        state.history.push_executed(EditorCommand::EditBuildingDef {
                            id: target_id.clone(),
                            before: Box::new(before),
                            after: Box::new(after),
                        });
                    }
                } else if let Some(before) = loaded.units.get(target_id).cloned() {
                    let mut after = before.clone();
                    after.visual.icon = Some(rel_icon.clone());
                    loaded.units.insert(target_id.clone(), after.clone());
                    state.modified_units.insert(target_id.clone());
                    state.history.push_executed(EditorCommand::EditUnitDef {
                        id: target_id.clone(),
                        before: Box::new(before),
                        after: Box::new(after),
                    });
                }
                let _ = world.apply_mod_hot_reload(loaded);
                state.validation_dirty = true;
                state.status_banner = format!(
                    "📸 Queued 96x72 cameo capture -> {} (`visual.icon = \"{rel_icon}\"`)",
                    out_path.display()
                );
            }
        }
    });

    if let Some(ref cap_status) = stage.last_cameo_capture_status {
        ui.colored_label(egui::Color32::from_rgb(135, 225, 175), cap_status);
    }

    // All 9 Animation Clip Selector Pills (`AnimClipMap`)
    ui.horizontal_wrapped(|ui| {
        ui.label("Stage Clip:");
        for anim_st in PreviewAnimState::ALL {
            let key = anim_st.stage_clip_key();
            let active = stage.active_anim_clip.eq_ignore_ascii_case(key);
            if ui.selectable_label(active, anim_st.label()).clicked() {
                stage.active_anim_clip = key.to_string();
                state.preview_anim_state = anim_st;
            }
        }
    });

    // Render Offscreen `512x384` Image with Interactive Orbit-Drag & Wheel-Zoom
    let avail_w = ui.available_width().clamp(200.0, 460.0);
    let max_h = if ui.ctx().screen_rect().height() <= 960.0 {
        150.0
    } else {
        345.0
    };
    let view_h = (avail_w * 0.75).min(max_h).round();
    let view_size = egui::vec2((view_h / 0.75).min(avail_w).round(), view_h);

    if let Some((_, tex_id)) = state.preview_stage_egui_tex {
        let (rect, resp) = ui.allocate_exact_size(view_size, egui::Sense::click_and_drag());
        ui.painter().image(
            tex_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.2, egui::Color32::from_rgb(75, 135, 195)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 8.0, rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            format!(
                "Layer 3 Turntable | Dist {:.1}m | Clip: {} (Drag: Orbit, Wheel: Zoom)",
                stage.orbit_distance, stage.active_anim_clip
            ),
            egui::FontId::proportional(10.5),
            egui::Color32::from_rgb(205, 228, 250),
        );

        if resp.dragged() {
            let delta = resp.drag_delta();
            stage.auto_turntable = false;
            stage.orbit_yaw += delta.x * 0.012;
            stage.orbit_pitch = (stage.orbit_pitch + delta.y * 0.010).clamp(0.08, 1.42);
        }
        if resp.hovered() {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.01 {
                stage.orbit_distance = (stage.orbit_distance - scroll_y * 0.015).clamp(1.8, 24.0);
            }
        }
    } else {
        ui.weak("Initializing 512x384 offscreen turntable texture...");
    }
}

fn show_socket_and_anim_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    preview_stage: Option<&mut PreviewStageState>,
) {
    match state.inspect_target.clone() {
        SdkInspectTarget::Unit(uid) => {
            let Some(before) = loaded.units.get(&uid).cloned() else {
                ui.weak("Inspected unit not found.");
                return;
            };
            let mut draft = before.clone();
            ui.horizontal(|ui| {
                ui.strong(format!("Rigging Unit: {} ({uid})", draft.name));
                if ui.button("💾 Save Sockets & Visual to .toml").clicked() {
                    if let Ok(path) = loaded.save_unit_def_toml(draft.clone()) {
                        state.modified_units.remove(&uid);
                        state.status_banner =
                            format!("Saved sockets/visual for '{uid}' -> {}", path.display());
                    }
                }
            });
            edit_visual_def_sockets_and_anims(ui, state, &mut draft.visual);
            if draft != before {
                if let Some(st) = preview_stage {
                    st.inspected_model_path = Some(draft.visual.model.clone());
                    st.sockets = draft.visual.sockets.clone();
                }
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
                ui.weak("Inspected building not found.");
                return;
            };
            let mut draft = before.clone();
            ui.horizontal(|ui| {
                ui.strong(format!("Rigging Building: {} ({bid})", draft.name));
                if ui.button("💾 Save Sockets & Visual to .toml").clicked() {
                    if let Ok(path) = loaded.save_building_def_toml(draft.clone()) {
                        state.modified_buildings.remove(&bid);
                        state.status_banner =
                            format!("Saved sockets/visual for '{bid}' -> {}", path.display());
                    }
                }
            });
            edit_visual_def_sockets_and_anims(ui, state, &mut draft.visual);
            if draft != before {
                if let Some(st) = preview_stage {
                    st.inspected_model_path = Some(draft.visual.model.clone());
                    st.sockets = draft.visual.sockets.clone();
                }
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
            ui.weak(
                "Select a Unit or Building in Asset Browser (Tab 1) to preview on the 3D Turntable and rig its weapon/VFX sockets.",
            );
        }
    }
}

fn edit_visual_def_sockets_and_anims(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    visual: &mut VisualDef,
) {
    ui.horizontal(|ui| {
        ui.label("3D Model Path:");
        ui.text_edit_singleline(&mut visual.model);
    });
    let mut icon_buf = visual.icon.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Cameo Icon PNG:");
        if ui.text_edit_singleline(&mut icon_buf).changed() {
            visual.icon = (!icon_buf.trim().is_empty()).then(|| icon_buf.trim().to_string());
        }
    });
    ui.horizontal(|ui| {
        ui.label("Scale XYZ:");
        ui.add(
            egui::DragValue::new(&mut visual.scale[0])
                .speed(0.05)
                .range(0.1..=6.0),
        );
        ui.add(
            egui::DragValue::new(&mut visual.scale[1])
                .speed(0.05)
                .range(0.1..=6.0),
        );
        ui.add(
            egui::DragValue::new(&mut visual.scale[2])
                .speed(0.05)
                .range(0.1..=6.0),
        );
        ui.checkbox(&mut visual.has_turret, "Turret");
        ui.checkbox(&mut visual.has_damage_states, "Damage States");
    });

    ui.strong("Named 3D Sockets (`VisualDef.sockets`):");
    ui.horizontal_wrapped(|ui| {
        for preset in ["Muzzle_01", "Muzzle_02", "Exhaust_01", "DamageSmoke_01"] {
            let exists = visual.sockets.iter().any(|s| s.name == preset);
            if !exists && ui.small_button(format!("+ {preset}")).clicked() {
                let default_off = match preset {
                    "Muzzle_01" => [0.0, 0.75, 1.35],
                    "Muzzle_02" => [0.28, 0.75, 1.35],
                    "Exhaust_01" => [-0.35, 0.55, -1.10],
                    _ => [0.0, 1.15, 0.0],
                };
                visual.sockets.push(SocketOffset {
                    name: preset.to_string(),
                    offset: default_off,
                });
            }
        }
    });

    let mut remove_sock: Option<usize> = None;
    for (idx, sock) in visual.sockets.iter_mut().enumerate() {
        ui.push_id(("sock_row", idx), |ui| {
            ui.horizontal(|ui| {
                ui.strong(&sock.name);
                ui.add(egui::Slider::new(&mut sock.offset[0], -4.0..=4.0).text("X"));
                ui.add(egui::Slider::new(&mut sock.offset[1], -1.0..=5.0).text("Y"));
                ui.add(egui::Slider::new(&mut sock.offset[2], -4.0..=4.0).text("Z"));
                if ui.small_button("✖").clicked() {
                    remove_sock = Some(idx);
                }
            });
        });
    }
    if let Some(i) = remove_sock {
        visual.sockets.remove(i);
    }

    ui.separator();
    ui.strong("Animation Clip Mapping (`VisualDef.animations` — 9 Clips):");
    ui.horizontal_wrapped(|ui| {
        for st in PreviewAnimState::ALL {
            ui.selectable_value(&mut state.preview_anim_state, st, st.label());
        }
    });
    let clip_slot = match state.preview_anim_state {
        PreviewAnimState::Idle => &mut visual.animations.idle,
        PreviewAnimState::Walk => &mut visual.animations.walk,
        PreviewAnimState::Prone => &mut visual.animations.prone,
        PreviewAnimState::Attack => &mut visual.animations.attack,
        PreviewAnimState::Deploy => &mut visual.animations.deploy,
        PreviewAnimState::Die => &mut visual.animations.die,
        PreviewAnimState::Constructing => &mut visual.animations.constructing,
        PreviewAnimState::Producing => &mut visual.animations.producing,
        PreviewAnimState::Damaged => &mut visual.animations.damaged,
    };
    let mut buf = clip_slot.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(format!("{} Clip Name:", state.preview_anim_state.label()));
        if ui.text_edit_singleline(&mut buf).changed() {
            *clip_slot = (!buf.trim().is_empty()).then(|| buf.trim().to_string());
        }
    });
}
