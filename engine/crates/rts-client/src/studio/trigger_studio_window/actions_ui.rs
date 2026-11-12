use super::conditions_ui::edit_objective_status_combo;
use bevy_egui::egui;
use rts_sdk::schema::{
    MissionObjectiveDef, ObjectiveStatus, ObjectiveTier, StackPolicy, StatusEffectDef,
    StatusVfxStyle, TriggerAction, TriggerOrderKind,
};

pub(crate) fn action_kind_label(act: &TriggerAction) -> &'static str {
    match act {
        TriggerAction::SpawnReinforcements { .. } => "SpawnReinforcements",
        TriggerAction::SpawnBuilding { .. } => "SpawnBuilding",
        TriggerAction::ChangeOwner { .. } => "ChangeOwner",
        TriggerAction::FireSupportPower { .. } => "FireSupportPower",
        TriggerAction::GrantCredits { .. } => "GrantCredits",
        TriggerAction::RevealArea { .. } => "RevealArea",
        TriggerAction::RevealAreaForPlayer { .. } => "RevealAreaForPlayer",
        TriggerAction::SetObjective { .. } => "SetObjective",
        TriggerAction::UpsertObjective { .. } => "UpsertObjective",
        TriggerAction::SetObjectiveStatus { .. } => "SetObjectiveStatus",
        TriggerAction::AddObjectiveProgress { .. } => "AddObjectiveProgress",
        TriggerAction::StartCountdown { .. } => "StartCountdown",
        TriggerAction::StopCountdown { .. } => "StopCountdown",
        TriggerAction::PlayRadarVideo { .. } => "PlayRadarVideo",
        TriggerAction::PlayCutscene { .. } => "PlayCutscene",
        TriggerAction::ExpandPlayableBounds { .. } => "ExpandPlayableBounds",
        TriggerAction::SetWeather { .. } => "SetWeather",
        TriggerAction::DeclareWinner { .. } => "DeclareWinner",
        TriggerAction::SetVariable { .. } => "SetVariable",
        TriggerAction::AddVariable { .. } => "AddVariable",
        TriggerAction::FocusCamera { .. } => "FocusCamera",
        TriggerAction::EvaTransmission { .. } => "EvaTransmission",
        TriggerAction::IssueOrder { .. } => "IssueOrder",
        TriggerAction::ApplyStatusEffect { .. } => "ApplyStatusEffect",
        TriggerAction::TeleportEntity { .. } => "TeleportEntity",
        TriggerAction::SpawnCaptureZone { .. } => "SpawnCaptureZone",
    }
}

pub(crate) fn default_status_effect_sample() -> StatusEffectDef {
    StatusEffectDef {
        id: "overdrive_buff".to_string(),
        duration_secs: 10.0,
        speed_multiplier: 1.25,
        damage_multiplier: 1.25,
        armor_multiplier: 1.0,
        stack_policy: StackPolicy::Refresh,
        max_stacks: 1,
        vfx_style: StatusVfxStyle::Ring,
        ..StatusEffectDef::default()
    }
}

pub(crate) fn edit_action_kind_switcher(
    ui: &mut egui::Ui,
    act: &mut TriggerAction,
    unit_ids: &[String],
    building_ids: &[String],
    cutscene_ids: &[String],
    objective_ids: &[String],
) {
    egui::ComboBox::from_id_salt("act_kind_cb")
        .selected_text(action_kind_label(act))
        .show_ui(ui, |ui| {
            if ui.selectable_label(false, "SpawnReinforcements").clicked() {
                *act = TriggerAction::SpawnReinforcements {
                    player_id: 1,
                    unit_id: unit_ids.first().cloned().unwrap_or_default(),
                    count: 3,
                    tile: [10, 10],
                };
            }
            if ui.selectable_label(false, "SpawnBuilding").clicked() {
                *act = TriggerAction::SpawnBuilding {
                    player_id: 1,
                    building_id: building_ids.first().cloned().unwrap_or_default(),
                    tile: [12, 12],
                };
            }
            if ui.selectable_label(false, "ChangeOwner").clicked() {
                *act = TriggerAction::ChangeOwner {
                    from_player_id: 0,
                    to_player_id: 1,
                    building_id: None,
                };
            }
            if ui.selectable_label(false, "FireSupportPower").clicked() {
                *act = TriggerAction::FireSupportPower {
                    player_id: 2,
                    power_id: "airstrike".to_string(),
                    target_tile: [16, 16],
                };
            }
            if ui.selectable_label(false, "GrantCredits").clicked() {
                *act = TriggerAction::GrantCredits {
                    player_id: 1,
                    amount: 1500,
                };
            }
            if ui.selectable_label(false, "RevealArea").clicked() {
                *act = TriggerAction::RevealArea {
                    tile: [16, 16],
                    radius: 8,
                };
            }
            if ui.selectable_label(false, "RevealAreaForPlayer").clicked() {
                *act = TriggerAction::RevealAreaForPlayer {
                    player_id: 1,
                    center: [24.0, 0.0, 24.0],
                    radius: 12.0,
                    duration_secs: 20,
                };
            }
            if ui.selectable_label(false, "SetObjective").clicked() {
                *act = TriggerAction::SetObjective {
                    text: "Capture the enemy command center.".to_string(),
                };
            }
            if ui.selectable_label(false, "UpsertObjective").clicked() {
                *act = TriggerAction::UpsertObjective {
                    objective: MissionObjectiveDef {
                        id: "obj_primary_1".to_string(),
                        tier: ObjectiveTier::Primary,
                        title: "Secure Forward Outpost".to_string(),
                        description: Some("Eliminate garrisoned defenders.".to_string()),
                        target_count: 1,
                        marker_tile: Some([16, 16]),
                    },
                    status: ObjectiveStatus::Active,
                };
            }
            if ui.selectable_label(false, "SetObjectiveStatus").clicked() {
                *act = TriggerAction::SetObjectiveStatus {
                    objective_id: objective_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "obj_primary_1".to_string()),
                    status: ObjectiveStatus::Completed,
                };
            }
            if ui.selectable_label(false, "AddObjectiveProgress").clicked() {
                *act = TriggerAction::AddObjectiveProgress {
                    objective_id: objective_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "obj_primary_1".to_string()),
                    delta: 1,
                };
            }
            if ui.selectable_label(false, "StartCountdown").clicked() {
                *act = TriggerAction::StartCountdown {
                    timer_id: "holdout_timer".to_string(),
                    label: "Reinforcements ETA".to_string(),
                    duration_secs: 180,
                };
            }
            if ui.selectable_label(false, "StopCountdown").clicked() {
                *act = TriggerAction::StopCountdown {
                    timer_id: "holdout_timer".to_string(),
                };
            }
            if ui.selectable_label(false, "PlayRadarVideo").clicked() {
                *act = TriggerAction::PlayRadarVideo {
                    video_path: "assets/videos/radar_alert.mp4".to_string(),
                };
            }
            if ui.selectable_label(false, "PlayCutscene").clicked() {
                *act = TriggerAction::PlayCutscene {
                    cutscene_id: cutscene_ids
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "intro_flyover".to_string()),
                };
            }
            if ui.selectable_label(false, "ExpandPlayableBounds").clicked() {
                *act = TriggerAction::ExpandPlayableBounds {
                    min_tile: [0, 0],
                    max_tile: [63, 63],
                };
            }
            if ui.selectable_label(false, "EvaTransmission").clicked() {
                *act = TriggerAction::EvaTransmission {
                    speaker: "EVA".to_string(),
                    text: "Reinforcements have arrived!".to_string(),
                    portrait: None,
                    audio_cue: None,
                    radar_video: None,
                };
            }
            if ui.selectable_label(false, "FocusCamera").clicked() {
                *act = TriggerAction::FocusCamera { tile: [16, 16] };
            }
            if ui.selectable_label(false, "SetWeather").clicked() {
                *act = TriggerAction::SetWeather {
                    preset: "IonStorm".to_string(),
                };
            }
            if ui.selectable_label(false, "DeclareWinner").clicked() {
                *act = TriggerAction::DeclareWinner { player_id: 1 };
            }
            if ui.selectable_label(false, "SetVariable").clicked() {
                *act = TriggerAction::SetVariable {
                    name: "phase".to_string(),
                    value: 2,
                };
            }
            if ui.selectable_label(false, "AddVariable").clicked() {
                *act = TriggerAction::AddVariable {
                    name: "kills".to_string(),
                    delta: 1,
                };
            }
            if ui.selectable_label(false, "IssueOrder").clicked() {
                *act = TriggerAction::IssueOrder {
                    tag: Some("strike_team".to_string()),
                    player_id: Some(2),
                    order: TriggerOrderKind::AttackMove,
                    target_tile: [16, 16],
                };
            }
            if ui.selectable_label(false, "ApplyStatusEffect").clicked() {
                *act = TriggerAction::ApplyStatusEffect {
                    tag: None,
                    player_id: Some(1),
                    tile: Some([16, 16]),
                    radius: 8.0,
                    status: default_status_effect_sample(),
                };
            }
            if ui.selectable_label(false, "TeleportEntity").clicked() {
                *act = TriggerAction::TeleportEntity {
                    tag: Some("hero_squad".to_string()),
                    player_id: Some(1),
                    from_tile: None,
                    radius: 6.0,
                    destination_tile: [20, 20],
                };
            }
            if ui.selectable_label(false, "SpawnCaptureZone").clicked() {
                *act = TriggerAction::SpawnCaptureZone {
                    zone_id: "cap_uplink".to_string(),
                    tile: [24, 24],
                    radius: 5.0,
                    initial_owner: None,
                    capture_time_secs: 8.0,
                };
            }
        });
}

pub(crate) fn edit_opt_string_inline(ui: &mut egui::Ui, label: &str, opt: &mut Option<String>) {
    let mut buf = opt.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.text_edit_singleline(&mut buf).changed() {
            let trimmed = buf.trim();
            *opt = (!trimmed.is_empty()).then(|| trimmed.to_string());
        }
    });
}

pub(crate) fn edit_trigger_action_fields(
    ui: &mut egui::Ui,
    act: &mut TriggerAction,
    unit_ids: &[String],
    building_ids: &[String],
    cutscene_ids: &[String],
    objective_ids: &[String],
) {
    match act {
        TriggerAction::SpawnReinforcements {
            player_id,
            unit_id,
            count,
            tile,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(player_id).range(0..=8).prefix("P"));
                egui::ComboBox::from_id_salt("act_unit_cb")
                    .selected_text(unit_id.as_str())
                    .show_ui(ui, |ui| {
                        for uid in unit_ids {
                            ui.selectable_value(unit_id, uid.clone(), uid);
                        }
                    });
                ui.add(egui::DragValue::new(count).range(1..=32).prefix("x"));
                ui.add(egui::DragValue::new(&mut tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut tile[1]).prefix("Z:"));
            });
        }
        TriggerAction::SpawnBuilding {
            player_id,
            building_id,
            tile,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(player_id).range(0..=8).prefix("P"));
                egui::ComboBox::from_id_salt("act_bldg_cb")
                    .selected_text(building_id.as_str())
                    .show_ui(ui, |ui| {
                        for bid in building_ids {
                            ui.selectable_value(building_id, bid.clone(), bid);
                        }
                    });
                ui.add(egui::DragValue::new(&mut tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut tile[1]).prefix("Z:"));
            });
        }
        TriggerAction::ChangeOwner {
            from_player_id,
            to_player_id,
            building_id,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(from_player_id)
                        .range(0..=8)
                        .prefix("From P"),
                );
                ui.add(
                    egui::DragValue::new(to_player_id)
                        .range(0..=8)
                        .prefix("To P"),
                );
            });
            edit_opt_string_inline(ui, "Building ID (empty=All):", building_id);
        }
        TriggerAction::FireSupportPower {
            player_id,
            power_id,
            target_tile,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(player_id).range(1..=8).prefix("P"));
                ui.label("Power:");
                ui.text_edit_singleline(power_id);
                ui.add(egui::DragValue::new(&mut target_tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut target_tile[1]).prefix("Z:"));
            });
        }
        TriggerAction::GrantCredits { player_id, amount } => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(player_id).range(1..=8).prefix("P"));
                ui.add(egui::DragValue::new(amount).range(50..=50000).prefix("$"));
            });
        }
        TriggerAction::RevealArea { tile, radius } => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut tile[1]).prefix("Z:"));
                ui.add(egui::DragValue::new(radius).range(2..=40).prefix("R:"));
            });
        }
        TriggerAction::RevealAreaForPlayer {
            player_id,
            center,
            radius,
            duration_secs,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(player_id).range(1..=8).prefix("P"));
                ui.add(egui::DragValue::new(&mut center[0]).prefix("WX:"));
                ui.add(egui::DragValue::new(&mut center[2]).prefix("WZ:"));
                ui.add(egui::DragValue::new(radius).range(1.0..=120.0).prefix("R:"));
                ui.add(
                    egui::DragValue::new(duration_secs)
                        .range(1..=3600)
                        .suffix("s"),
                );
            });
        }
        TriggerAction::SetObjective { text } => {
            ui.text_edit_singleline(text);
        }
        TriggerAction::UpsertObjective { objective, status } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("ID:");
                ui.text_edit_singleline(&mut objective.id);
                egui::ComboBox::from_id_salt("act_obj_tier")
                    .selected_text(format!("{:?}", objective.tier))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut objective.tier, ObjectiveTier::Primary, "Primary");
                        ui.selectable_value(
                            &mut objective.tier,
                            ObjectiveTier::Secondary,
                            "Secondary",
                        );
                        ui.selectable_value(&mut objective.tier, ObjectiveTier::Bonus, "Bonus");
                    });
                edit_objective_status_combo(ui, "act_upsert_obj_st", status);
            });
            ui.horizontal(|ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut objective.title);
                ui.add(
                    egui::DragValue::new(&mut objective.target_count)
                        .range(0..=500)
                        .prefix("Target: "),
                );
            });
            edit_opt_string_inline(ui, "Description:", &mut objective.description);
        }
        TriggerAction::SetObjectiveStatus {
            objective_id,
            status,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Objective:");
                if objective_ids.is_empty() {
                    ui.text_edit_singleline(objective_id);
                } else {
                    egui::ComboBox::from_id_salt("act_set_obj_cb")
                        .selected_text(objective_id.as_str())
                        .show_ui(ui, |ui| {
                            for oid in objective_ids {
                                ui.selectable_value(objective_id, oid.clone(), oid);
                            }
                        });
                }
                edit_objective_status_combo(ui, "act_set_obj_st", status);
            });
        }
        TriggerAction::AddObjectiveProgress {
            objective_id,
            delta,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Objective:");
                ui.text_edit_singleline(objective_id);
                ui.add(egui::DragValue::new(delta).range(1..=100).prefix("+"));
            });
        }
        TriggerAction::StartCountdown {
            timer_id,
            label,
            duration_secs,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("ID:");
                ui.text_edit_singleline(timer_id);
                ui.label("Label:");
                ui.text_edit_singleline(label);
                ui.add(
                    egui::DragValue::new(duration_secs)
                        .range(1..=7200)
                        .suffix("s"),
                );
            });
        }
        TriggerAction::StopCountdown { timer_id } => {
            ui.horizontal(|ui| {
                ui.label("Timer ID:");
                ui.text_edit_singleline(timer_id);
            });
        }
        TriggerAction::PlayRadarVideo { video_path } => {
            ui.horizontal(|ui| {
                ui.label("MP4 Path:");
                ui.text_edit_singleline(video_path);
            });
        }
        TriggerAction::PlayCutscene { cutscene_id } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Cutscene:");
                if cutscene_ids.is_empty() {
                    ui.text_edit_singleline(cutscene_id);
                } else {
                    egui::ComboBox::from_id_salt("act_cutscene_cb")
                        .selected_text(cutscene_id.as_str())
                        .show_ui(ui, |ui| {
                            for cid in cutscene_ids {
                                ui.selectable_value(cutscene_id, cid.clone(), cid);
                            }
                        });
                }
            });
        }
        TriggerAction::ExpandPlayableBounds { min_tile, max_tile } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(&mut min_tile[0]).prefix("MinX:"));
                ui.add(egui::DragValue::new(&mut min_tile[1]).prefix("MinZ:"));
                ui.add(egui::DragValue::new(&mut max_tile[0]).prefix("MaxX:"));
                ui.add(egui::DragValue::new(&mut max_tile[1]).prefix("MaxZ:"));
            });
        }
        TriggerAction::EvaTransmission {
            speaker,
            text,
            portrait,
            audio_cue,
            radar_video,
        } => {
            ui.horizontal(|ui| {
                ui.label("Speaker:");
                ui.text_edit_singleline(speaker);
            });
            ui.text_edit_singleline(text);
            edit_opt_string_inline(ui, "Portrait:", portrait);
            edit_opt_string_inline(ui, "Audio Cue:", audio_cue);
            edit_opt_string_inline(ui, "Radar Video:", radar_video);
        }
        TriggerAction::FocusCamera { tile } => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut tile[1]).prefix("Z:"));
            });
        }
        TriggerAction::SetWeather { preset } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Preset:");
                for p in ["Day", "Dusk", "Night", "IonStorm"] {
                    if ui
                        .selectable_label(preset.eq_ignore_ascii_case(p), p)
                        .clicked()
                    {
                        *preset = p.to_string();
                    }
                }
                ui.text_edit_singleline(preset);
            });
        }
        TriggerAction::DeclareWinner { player_id } => {
            ui.add(
                egui::DragValue::new(player_id)
                    .range(1..=8)
                    .prefix("Winner P"),
            );
        }
        TriggerAction::SetVariable { name, value } => {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(name);
                ui.add(egui::DragValue::new(value));
            });
        }
        TriggerAction::AddVariable { name, delta } => {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(name);
                ui.add(egui::DragValue::new(delta));
            });
        }
        TriggerAction::IssueOrder {
            tag,
            player_id,
            order,
            target_tile,
        } => {
            edit_opt_string_inline(ui, "Tag (opt):", tag);
            ui.horizontal_wrapped(|ui| {
                let mut pid = player_id.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut pid)
                            .range(0..=8)
                            .prefix("P(0=Any):"),
                    )
                    .changed()
                {
                    *player_id = (pid > 0).then_some(pid);
                }
                egui::ComboBox::from_id_salt("act_order_kind")
                    .selected_text(format!("{order:?}"))
                    .show_ui(ui, |ui| {
                        for k in [
                            TriggerOrderKind::Move,
                            TriggerOrderKind::AttackMove,
                            TriggerOrderKind::Patrol,
                            TriggerOrderKind::Guard,
                            TriggerOrderKind::Stop,
                            TriggerOrderKind::HoldPosition,
                        ] {
                            ui.selectable_value(order, k, format!("{k:?}"));
                        }
                    });
                ui.add(egui::DragValue::new(&mut target_tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut target_tile[1]).prefix("Z:"));
            });
        }
        TriggerAction::ApplyStatusEffect {
            tag,
            player_id,
            tile: _,
            radius,
            status,
        } => {
            edit_opt_string_inline(ui, "Tag (opt):", tag);
            ui.horizontal_wrapped(|ui| {
                let mut pid = player_id.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut pid)
                            .range(0..=8)
                            .prefix("P(0=Any):"),
                    )
                    .changed()
                {
                    *player_id = (pid > 0).then_some(pid);
                }
                ui.add(egui::DragValue::new(radius).range(0.0..=80.0).prefix("R:"));
                ui.label("Status ID:");
                ui.text_edit_singleline(&mut status.id);
                ui.add(
                    egui::DragValue::new(&mut status.duration_secs)
                        .range(0.5..=300.0)
                        .suffix("s"),
                );
            });
        }
        TriggerAction::TeleportEntity {
            tag,
            player_id,
            from_tile: _,
            radius,
            destination_tile,
        } => {
            edit_opt_string_inline(ui, "Tag (opt):", tag);
            ui.horizontal_wrapped(|ui| {
                let mut pid = player_id.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut pid)
                            .range(0..=8)
                            .prefix("P(0=Any):"),
                    )
                    .changed()
                {
                    *player_id = (pid > 0).then_some(pid);
                }
                ui.add(egui::DragValue::new(radius).range(0.0..=64.0).prefix("R:"));
                ui.add(egui::DragValue::new(&mut destination_tile[0]).prefix("DstX:"));
                ui.add(egui::DragValue::new(&mut destination_tile[1]).prefix("DstZ:"));
            });
        }
        TriggerAction::SpawnCaptureZone {
            zone_id,
            tile,
            radius,
            initial_owner,
            capture_time_secs,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.label("Zone ID:");
                ui.text_edit_singleline(zone_id);
                ui.add(egui::DragValue::new(&mut tile[0]).prefix("X:"));
                ui.add(egui::DragValue::new(&mut tile[1]).prefix("Z:"));
                ui.add(egui::DragValue::new(radius).range(1.0..=30.0).prefix("R:"));
                ui.add(
                    egui::DragValue::new(capture_time_secs)
                        .range(1.0..=120.0)
                        .suffix("s"),
                );
                let mut owner = initial_owner.unwrap_or(0);
                if ui
                    .add(
                        egui::DragValue::new(&mut owner)
                            .range(0..=8)
                            .prefix("Owner(0=Neutral):"),
                    )
                    .changed()
                {
                    *initial_owner = (owner > 0).then_some(owner);
                }
            });
        }
    }
}
