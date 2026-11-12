use crate::audio::SoundSettingsResource;
use crate::rendering::VideoPlaybackResource;
use crate::studio::asset_browser::collect_mod_video_paths;
use crate::studio::history::EditorCommand;
use crate::studio::{SdkInspectTarget, StudioState};
use bevy_egui::egui;
use rts_asset_pipeline::discovery::{import_modern_asset, ModernAssetImportOptions};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::AudioBusKind;
use rts_simulation::world::SimWorld;
use std::path::Path;

pub(super) fn show_modern_asset_importer_panel(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    ui.horizontal(|ui| {
        ui.label("Source File:");
        ui.text_edit_singleline(&mut state.modern_import_source_path);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Asset ID (optional):");
        ui.text_edit_singleline(&mut state.modern_import_asset_id);
        ui.checkbox(&mut state.modern_import_as_building, "As Building");
        ui.checkbox(&mut state.modern_import_bake_cameo, "Bake 96x72 Cameo");
        ui.checkbox(&mut state.modern_import_optimize_glb, "Optimize .glb");
    });

    let can_import = !state.modern_import_source_path.trim().is_empty();
    if ui
        .add_enabled(
            can_import,
            egui::Button::new("📥 Import & Copy into Active Mod"),
        )
        .clicked()
    {
        let src_path = Path::new(state.modern_import_source_path.trim());
        let opts = ModernAssetImportOptions {
            asset_id: (!state.modern_import_asset_id.trim().is_empty())
                .then(|| state.modern_import_asset_id.trim().to_string()),
            as_building: state.modern_import_as_building,
            bake_cameo: state.modern_import_bake_cameo,
            optimize_glb: state.modern_import_optimize_glb,
        };
        match import_modern_asset(src_path, &loaded.root_dir, &opts) {
            Ok(rep) => {
                let mut summary = format!(
                    "✔ Imported '{}' ({}) -> {}\n",
                    rep.asset_id,
                    rep.kind.label(),
                    rep.relative_asset_path
                );
                if let Some(fp) = rep.suggested_footprint {
                    summary.push_str(&format!("Suggested Footprint: {}x{}\n", fp[0], fp[1]));
                }
                if !rep.detected_sockets.is_empty() {
                    let socks: Vec<&str> = rep
                        .detected_sockets
                        .iter()
                        .map(|(n, _)| n.as_str())
                        .collect();
                    summary.push_str(&format!("Detected Sockets: {}\n", socks.join(", ")));
                }
                if !rep.animation_clips.is_empty() {
                    summary.push_str(&format!(
                        "Animation Clips: {}\n",
                        rep.animation_clips.join(", ")
                    ));
                }
                if let Some(ref cameo) = rep.baked_cameo_path {
                    summary.push_str(&format!("Baked Cameo: {}\n", cameo.display()));
                }
                if !rep.toml_snippet.trim().is_empty() {
                    summary.push_str("\n--- Generated Visual TOML Snippet ---\n");
                    summary.push_str(&rep.toml_snippet);
                }

                // Optionally bind directly onto currently inspected Unit/Building if VisualDef was produced
                if let Some(vdef) = rep.visual_def {
                    match state.inspect_target.clone() {
                        SdkInspectTarget::Unit(uid) => {
                            if let Some(before) = loaded.units.get(&uid).cloned() {
                                let mut after = before.clone();
                                after.visual = vdef;
                                loaded.units.insert(uid.clone(), after.clone());
                                state.modified_units.insert(uid.clone());
                                state.cameo_egui_textures.remove(&uid);
                                state.history.push_executed(EditorCommand::EditUnitDef {
                                    id: uid,
                                    before: Box::new(before),
                                    after: Box::new(after),
                                });
                            }
                        }
                        SdkInspectTarget::Building(bid) => {
                            if let Some(before) = loaded.buildings.get(&bid).cloned() {
                                let mut after = before.clone();
                                after.visual = vdef;
                                loaded.buildings.insert(bid.clone(), after.clone());
                                state.modified_buildings.insert(bid.clone());
                                state.cameo_egui_textures.remove(&bid);
                                state.history.push_executed(EditorCommand::EditBuildingDef {
                                    id: bid,
                                    before: Box::new(before),
                                    after: Box::new(after),
                                });
                            }
                        }
                        _ => {}
                    }
                }

                let _ = world.apply_mod_hot_reload(loaded);
                state.cameo_egui_textures.remove(&rep.asset_id);
                state.validation_dirty = true;
                state.status_banner = format!(
                    "✔ Imported modern asset '{}' ({})",
                    rep.asset_id, rep.relative_asset_path
                );
                state.modern_import_last_report = Some(summary);
            }
            Err(err) => {
                let msg = format!("✖ Modern asset import failed: {err:#}");
                state.status_banner = msg.clone();
                state.modern_import_last_report = Some(msg);
            }
        }
    }

    if let Some(ref rep_txt) = state.modern_import_last_report {
        egui::ScrollArea::vertical()
            .id_salt("modern_asset_import_report_scroll")
            .max_height(130.0)
            .show(ui, |ui| {
                ui.monospace(rep_txt);
            });
    }
}

pub(super) fn show_audio_auditioner(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &LoadedMod,
    sound_res: &mut SoundSettingsResource,
) {
    for (bus, lbl) in [
        (AudioBusKind::Master, "Master"),
        (AudioBusKind::Music, "Music"),
        (AudioBusKind::CombatSfx, "Combat SFX"),
        (AudioBusKind::UnitVoice, "Unit Voice"),
        (AudioBusKind::Eva, "EVA"),
    ] {
        let mut vol = sound_res.bus_volume(bus);
        if ui
            .add(egui::Slider::new(&mut vol, 0.0..=1.0).text(lbl))
            .changed()
        {
            sound_res.set_bus_volume(bus, vol);
            sound_res.save();
        }
    }
    ui.horizontal_wrapped(|ui| {
        if let Some(first_track) = loaded.audio.music_tracks.first() {
            if ui
                .small_button(format!("▶ Music: {}", first_track.display_name))
                .clicked()
            {
                sound_res.play_music_track(&first_track.id);
                state.status_banner = format!("Auditioning track '{}'", first_track.display_name);
            }
        }
        if ui.small_button("⏹ Stop Music").clicked() {
            sound_res.stop_music();
        }
        if let Some((cue_id, _)) = loaded.audio.eva_cues.iter().next() {
            if ui.small_button(format!("▶ EVA: {cue_id}")).clicked() {
                sound_res.push_eva_announcement("shared", cue_id);
            }
        }
        if let Some((uid, _)) = loaded.audio.unit_voices.iter().next() {
            if ui.small_button(format!("▶ Voice: {uid}")).clicked() {
                sound_res.trigger_unit_voice(uid, "select");
            }
        }
    });
}

pub(super) fn show_video_auditioner(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &LoadedMod,
    video_res: Option<&mut VideoPlaybackResource>,
) {
    let Some(vres) = video_res else {
        ui.weak("VideoPlaybackResource unavailable in headless mode.");
        return;
    };

    ui.horizontal_wrapped(|ui| {
        if ui.button("▶ Play Briefing Signal").clicked() {
            let clip_rel = collect_mod_video_paths(loaded)
                .into_iter()
                .next()
                .unwrap_or_else(|| "assets/videos/studio_test_signal.mp4".to_string());
            let full = loaded.resolve_asset_path(&clip_rel);
            vres.play_file(&full, &clip_rel);
            state.status_banner = format!("In-engine FMV auditioning '{clip_rel}'");
        }
        if ui.button("⏯ Pause / Resume").clicked() {
            vres.toggle_pause();
        }
        if ui.button("⏹ Stop").clicked() {
            vres.stop();
        }
    });

    ui.add(egui::ProgressBar::new(vres.progress_01()).text(format!(
        "{} ({:.1}s / {:.1}s)",
        if vres.active_title.is_empty() {
            "Idle"
        } else {
            vres.active_title.as_str()
        },
        vres.elapsed_secs,
        vres.duration_secs
    )));

    if let Some((_, tex_id)) = state.video_egui_tex {
        ui.image(egui::load::SizedTexture::new(
            tex_id,
            egui::vec2(280.0, 158.0),
        ));
    }
    if let Some(sub) = vres.current_subtitle.as_deref().filter(|s| !s.is_empty()) {
        ui.colored_label(egui::Color32::from_rgb(175, 225, 255), sub);
    }
}
