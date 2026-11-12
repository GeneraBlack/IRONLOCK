use super::edit_csv_strings;
use crate::studio::history::EditorCommand;
use crate::studio::StudioState;
use bevy_egui::egui;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    CameraCutsceneDef, CameraKeyframe, MissionObjectiveDef, MusicMood, MusicTrackDef,
    ObjectiveTier, UnitVoiceSpec,
};
use rts_simulation::world::SimWorld;

pub fn show_active_map_objectives_and_cutscenes_editor(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) {
    let before_map = loaded.entry_map.clone();
    let mut map_draft = before_map.clone();

    egui::CollapsingHeader::new(format!(
        "Active Map '{}' Objectives ({})",
        map_draft.id,
        map_draft.objectives.len()
    ))
    .default_open(false)
    .show(ui, |ui| {
        if ui.small_button("+ Add Structured Objective").clicked() {
            let n = map_draft.objectives.len() + 1;
            map_draft.objectives.push(MissionObjectiveDef {
                id: format!("obj_{n}"),
                tier: ObjectiveTier::Primary,
                title: format!("Primary Objective {n}"),
                description: None,
                target_count: 0,
                marker_tile: None,
            });
        }
        let mut rem_obj = None;
        for (idx, obj) in map_draft.objectives.iter_mut().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut obj.id)
                        .id_salt(("mobj_id", idx))
                        .desired_width(80.0),
                );
                egui::ComboBox::from_id_salt(("mobj_tier", idx))
                    .selected_text(format!("{:?}", obj.tier))
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for t in [
                            ObjectiveTier::Primary,
                            ObjectiveTier::Secondary,
                            ObjectiveTier::Bonus,
                        ] {
                            ui.selectable_value(&mut obj.tier, t, format!("{t:?}"));
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut obj.title)
                        .id_salt(("mobj_title", idx))
                        .desired_width(145.0),
                );
                ui.add(
                    egui::DragValue::new(&mut obj.target_count)
                        .range(0..=999)
                        .prefix("Cnt "),
                );
                let mut has_marker = obj.marker_tile.is_some();
                if ui.checkbox(&mut has_marker, "Pin").changed() {
                    obj.marker_tile = if has_marker { Some([16, 16]) } else { None };
                }
                if let Some(tile) = &mut obj.marker_tile {
                    ui.add(egui::DragValue::new(&mut tile[0]).prefix("X:"));
                    ui.add(egui::DragValue::new(&mut tile[1]).prefix("Z:"));
                }
                if ui.small_button("✖").clicked() {
                    rem_obj = Some(idx);
                }
            });
        }
        if let Some(i) = rem_obj {
            map_draft.objectives.remove(i);
        }
    });

    egui::CollapsingHeader::new(format!(
        "Active Map '{}' Camera Cutscenes ({})",
        map_draft.id,
        map_draft.cutscenes.len()
    ))
    .default_open(false)
    .show(ui, |ui| {
        if ui.small_button("+ Add Camera Cutscene").clicked() {
            let n = map_draft.cutscenes.len() + 1;
            map_draft.cutscenes.push(CameraCutsceneDef {
                id: format!("cutscene_{n}"),
                skippable: true,
                letterbox: true,
                keyframes: vec![
                    CameraKeyframe {
                        time_secs: 0.0,
                        focus: [24.0, 0.0, 24.0],
                        distance: 42.0,
                        pitch_deg: 55.0,
                        yaw_deg: 0.0,
                    },
                    CameraKeyframe {
                        time_secs: 4.0,
                        focus: [48.0, 0.0, 48.0],
                        distance: 32.0,
                        pitch_deg: 50.0,
                        yaw_deg: 25.0,
                    },
                ],
            });
        }
        let mut rem_cs = None;
        for (cs_idx, cs) in map_draft.cutscenes.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut cs.id)
                            .id_salt(("mcs_id", cs_idx))
                            .desired_width(110.0),
                    );
                    ui.checkbox(&mut cs.skippable, "Skippable");
                    ui.checkbox(&mut cs.letterbox, "Letterbox");
                    if ui.small_button("▶ Preview Cutscene").clicked() {
                        state.spline_tools.selected_cutscene_idx = cs_idx;
                        state.spline_tools.active_cutscene_preview = Some((cs.id.clone(), 0.0));
                        state.status_banner = format!("▶ Previewing cutscene '{}'...", cs.id);
                    }
                    if ui.small_button("+ Keyframe").clicked() {
                        let next_t = cs
                            .keyframes
                            .last()
                            .map(|k| k.time_secs + 2.5)
                            .unwrap_or(0.0);
                        cs.keyframes.push(CameraKeyframe {
                            time_secs: next_t,
                            focus: [32.0, 0.0, 32.0],
                            distance: 36.0,
                            pitch_deg: 55.0,
                            yaw_deg: 0.0,
                        });
                    }
                    if ui.small_button("✖").clicked() {
                        rem_cs = Some(cs_idx);
                    }
                });
                let mut rem_kf = None;
                for (kf_idx, kf) in cs.keyframes.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("KF#{kf_idx}"));
                        ui.add(
                            egui::DragValue::new(&mut kf.time_secs)
                                .range(0.0..=300.0)
                                .speed(0.25)
                                .prefix("t="),
                        );
                        ui.add(
                            egui::DragValue::new(&mut kf.focus[0])
                                .speed(0.5)
                                .prefix("X:"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut kf.focus[1])
                                .speed(0.25)
                                .prefix("Y:"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut kf.focus[2])
                                .speed(0.5)
                                .prefix("Z:"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut kf.distance)
                                .range(8.0..=140.0)
                                .speed(0.5)
                                .prefix("Dist:"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut kf.yaw_deg)
                                .range(-180.0..=180.0)
                                .speed(1.0)
                                .prefix("Yaw:"),
                        );
                        if ui.small_button("✖").clicked() {
                            rem_kf = Some(kf_idx);
                        }
                    });
                }
                if let Some(k) = rem_kf {
                    cs.keyframes.remove(k);
                }
            });
        }
        if let Some(c) = rem_cs {
            map_draft.cutscenes.remove(c);
        }
    });

    if map_draft != before_map {
        loaded.entry_map = map_draft.clone();
        loaded
            .maps
            .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
        let _ = world;
        state.validation_dirty = true;
        state.history.push_executed(EditorCommand::EditMapDef {
            label: format!("Edit Map '{}' Objectives/Cutscenes", map_draft.id),
            before: Box::new(before_map),
            after: Box::new(map_draft),
        });
    }
}

pub(super) fn show_audio_catalog_inspector(
    ui: &mut egui::Ui,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
) {
    let before = loaded.audio.clone();
    let mut draft = before.clone();

    ui.horizontal(|ui| {
        ui.strong("AudioCatalog (`audio/audio.toml`)");
        if state.audio_modified {
            ui.colored_label(egui::Color32::from_rgb(245, 195, 75), "● Unsaved");
        }
        if ui.button("💾 Save audio.toml").clicked() {
            match loaded.save_audio_catalog(draft.clone()) {
                Ok(path) => {
                    state.audio_modified = false;
                    state.status_banner = format!("Saved AudioCatalog -> {}", path.display());
                }
                Err(err) => {
                    state.status_banner = format!("Save AudioCatalog error: {err}");
                }
            }
        }
    });
    ui.separator();

    egui::CollapsingHeader::new(format!("Music Tracks ({})", draft.music_tracks.len()))
        .default_open(true)
        .show(ui, |ui| {
            if ui.small_button("+ Add Music Track").clicked() {
                let n = draft.music_tracks.len() + 1;
                draft.music_tracks.push(MusicTrackDef {
                    id: format!("track_{n:02}"),
                    display_name: format!("Track {n}"),
                    path: "assets/audio/music/track.ogg".to_string(),
                    mood: MusicMood::Combat,
                    factions: Vec::new(),
                    faction_tags: Vec::new(),
                    duration_hint_secs: None,
                });
            }
            let mut rem_tr = None;
            for (idx, tr) in draft.music_tracks.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut tr.id)
                                .id_salt(("mtr_id", idx))
                                .desired_width(90.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut tr.display_name)
                                .id_salt(("mtr_nm", idx))
                                .desired_width(120.0),
                        );
                        egui::ComboBox::from_id_salt(("mtr_mood", idx))
                            .selected_text(format!("{:?}", tr.mood))
                            .width(85.0)
                            .show_ui(ui, |ui| {
                                for mood in [
                                    MusicMood::Peace,
                                    MusicMood::Combat,
                                    MusicMood::Menu,
                                    MusicMood::Score,
                                ] {
                                    ui.selectable_value(&mut tr.mood, mood, format!("{mood:?}"));
                                }
                            });
                        if ui.small_button("✖").clicked() {
                            rem_tr = Some(idx);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Path:");
                        ui.add(
                            egui::TextEdit::singleline(&mut tr.path)
                                .id_salt(("mtr_p", idx))
                                .desired_width(180.0),
                        );
                        ui.label("Faction Tags:");
                        edit_csv_strings(
                            ui,
                            ("mtr_ftag", idx),
                            &mut tr.faction_tags,
                            "allies, soviets",
                        );
                    });
                });
            }
            if let Some(i) = rem_tr {
                draft.music_tracks.remove(i);
            }
        });

    egui::CollapsingHeader::new(format!("EVA Cues ({})", draft.eva_cues.len()))
        .default_open(false)
        .show(ui, |ui| {
            let keys: Vec<String> = draft.eva_cues.keys().cloned().collect();
            let mut rem_k = None;
            for k in keys {
                ui.horizontal(|ui| {
                    ui.monospace(&k);
                    if let Some(p) = draft.eva_cues.get_mut(&k) {
                        ui.text_edit_singleline(p);
                    }
                    if ui.small_button("✖").clicked() {
                        rem_k = Some(k);
                    }
                });
            }
            if let Some(r) = rem_k {
                draft.eva_cues.remove(&r);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_prereq_buf)
                        .id_salt("aud_new_eva")
                        .desired_width(125.0)
                        .hint_text("cue_key (e.g. low_power)"),
                );
                if ui.small_button("+ Add EVA Cue").clicked() {
                    let k = state.new_prereq_buf.trim().to_string();
                    if !k.is_empty() {
                        draft
                            .eva_cues
                            .insert(k, "assets/audio/eva/cue.ogg".to_string());
                        state.new_prereq_buf.clear();
                    }
                }
            });
        });

    egui::CollapsingHeader::new(format!(
        "Faction EVA Cue Overrides ({})",
        draft.faction_eva_cues.len()
    ))
    .default_open(false)
    .show(ui, |ui| {
        for fid in loaded.factions.keys() {
            let fmap = draft.faction_eva_cues.entry(fid.clone()).or_default();
            ui.strong(format!("Faction '{fid}' ({})", fmap.len()));
            let ckeys: Vec<String> = fmap.keys().cloned().collect();
            let mut rem_ck = None;
            for ck in ckeys {
                ui.horizontal(|ui| {
                    ui.monospace(&ck);
                    if let Some(path) = fmap.get_mut(&ck) {
                        ui.text_edit_singleline(path);
                    }
                    if ui.small_button("✖").clicked() {
                        rem_ck = Some(ck);
                    }
                });
            }
            if let Some(r) = rem_ck {
                fmap.remove(&r);
            }
            if ui.small_button(format!("+ Add '{fid}' Cue")).clicked() {
                fmap.insert(
                    "unit_ready".to_string(),
                    format!("assets/audio/eva/{fid}_unit_ready.ogg"),
                );
            }
        }
        draft.faction_eva_cues.retain(|_, m| !m.is_empty());
    });

    egui::CollapsingHeader::new(format!("Weapon SFX ({})", draft.weapon_sfx.len()))
        .default_open(false)
        .show(ui, |ui| {
            let keys: Vec<String> = draft.weapon_sfx.keys().cloned().collect();
            let mut rem_w = None;
            for k in keys {
                ui.horizontal(|ui| {
                    ui.monospace(&k);
                    if let Some(p) = draft.weapon_sfx.get_mut(&k) {
                        ui.text_edit_singleline(p);
                    }
                    if ui.small_button("✖").clicked() {
                        rem_w = Some(k);
                    }
                });
            }
            if let Some(r) = rem_w {
                draft.weapon_sfx.remove(&r);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_buildable_buf)
                        .id_salt("aud_new_wsfx")
                        .desired_width(125.0)
                        .hint_text("weapon_id"),
                );
                if ui.small_button("+ Add Weapon SFX").clicked() {
                    let wid = state.new_buildable_buf.trim().to_string();
                    if !wid.is_empty() {
                        draft
                            .weapon_sfx
                            .insert(wid.clone(), format!("assets/audio/sfx/{wid}.ogg"));
                        state.new_buildable_buf.clear();
                    }
                }
            });
        });

    egui::CollapsingHeader::new(format!("Unit Voice Sets ({})", draft.unit_voices.len()))
        .default_open(false)
        .show(ui, |ui| {
            let ukeys: Vec<String> = draft.unit_voices.keys().cloned().collect();
            let mut rem_u = None;
            for uk in ukeys {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&uk);
                        if ui.small_button("✖").clicked() {
                            rem_u = Some(uk.clone());
                        }
                    });
                    if let Some(spec) = draft.unit_voices.get_mut(&uk) {
                        ui.horizontal(|ui| {
                            ui.label("Select:");
                            edit_csv_strings(ui, ("uv_sel", &uk), &mut spec.select, "sel1.ogg");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Move:");
                            edit_csv_strings(ui, ("uv_mov", &uk), &mut spec.move_order, "mov1.ogg");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Attack:");
                            edit_csv_strings(
                                ui,
                                ("uv_atk", &uk),
                                &mut spec.attack_order,
                                "atk1.ogg",
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Death:");
                            edit_csv_strings(ui, ("uv_die", &uk), &mut spec.death, "die1.ogg");
                        });
                    }
                });
            }
            if let Some(r) = rem_u {
                draft.unit_voices.remove(&r);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_res_cost_key)
                        .id_salt("aud_new_uvoice")
                        .desired_width(125.0)
                        .hint_text("unit_id"),
                );
                if ui.small_button("+ Add Unit Voice Set").clicked() {
                    let uid = state.new_res_cost_key.trim().to_string();
                    if !uid.is_empty() {
                        draft.unit_voices.insert(uid, UnitVoiceSpec::default());
                        state.new_res_cost_key.clear();
                    }
                }
            });
        });

    if draft != before {
        loaded.audio = draft.clone();
        state.audio_modified = true;
        state.validation_dirty = true;
        state
            .history
            .push_executed(EditorCommand::EditAudioCatalog {
                before: Box::new(before),
                after: Box::new(draft),
            });
    }
}
