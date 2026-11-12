use super::traits_visuals::validate_status_effect_refs;
use super::{check_existing_asset_binary, ValidationReport};
use crate::loader::LoadedMod;
use crate::schema::{TriggerAction, TriggerCondition};
use std::collections::BTreeSet;

pub(crate) fn validate_trigger_condition(
    cond: &TriggerCondition,
    map_id: &str,
    tz_id: &str,
    zone_ids: &BTreeSet<&str>,
    objective_ids: &BTreeSet<&str>,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) -> bool {
    match cond {
        TriggerCondition::EnteredZone { .. } => true,
        TriggerCondition::PeriodicTimer { interval_secs, .. } => {
            if *interval_secs == 0 {
                report.push_error(
                    "INVALID_TRIGGER_TIMER",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' PeriodicTimer must have interval_secs > 0"
                    ),
                );
            }
            false
        }
        TriggerCondition::TaggedEntityEnteredZone { zone_id, .. } => {
            if !zone_ids.is_empty() && !zone_ids.contains(zone_id.as_str()) {
                report.push_error(
                    "UNKNOWN_TRIGGER_ZONE",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' references unknown zone_id '{zone_id}'"
                    ),
                );
            }
            false
        }
        TriggerCondition::ObjectiveStatusIs { objective_id, .. } => {
            if !objective_ids.is_empty() && !objective_ids.contains(objective_id.as_str()) {
                report.push_error(
                    "UNKNOWN_OBJECTIVE",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' references unknown objective_id '{objective_id}'"
                    ),
                );
            }
            false
        }
        TriggerCondition::BuildingCaptured { building_id, .. }
        | TriggerCondition::BuildingDestroyed { building_id, .. } => {
            if !loaded.buildings.contains_key(building_id) {
                report.push_error(
                    "UNKNOWN_TRIGGER_BUILDING",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' references unknown building_id '{building_id}'"
                    ),
                );
            }
            false
        }
        TriggerCondition::All { conditions } | TriggerCondition::Any { conditions } => {
            let mut uses_zone = false;
            for sub in conditions {
                if validate_trigger_condition(
                    sub,
                    map_id,
                    tz_id,
                    zone_ids,
                    objective_ids,
                    loaded,
                    report,
                ) {
                    uses_zone = true;
                }
            }
            uses_zone
        }
        TriggerCondition::Not { condition } => validate_trigger_condition(
            condition,
            map_id,
            tz_id,
            zone_ids,
            objective_ids,
            loaded,
            report,
        ),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_trigger_action_on_map<F>(
    act: &TriggerAction,
    map_id: &str,
    tz_id: &str,
    w: i32,
    h: i32,
    objective_ids: &BTreeSet<&str>,
    cutscene_ids: &BTreeSet<&str>,
    defined_support_powers: &BTreeSet<&str>,
    loaded: &LoadedMod,
    check_unit_placement: &F,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) where
    F: Fn(&str, [i32; 2], &str, &mut ValidationReport),
{
    let tile_in_bounds = |tile: [i32; 2]| {
        w <= 0 || h <= 0 || (tile[0] >= 0 && tile[0] < w && tile[1] >= 0 && tile[1] < h)
    };

    match act {
        TriggerAction::SpawnReinforcements { unit_id, tile, .. } => {
            if !loaded.units.contains_key(unit_id) {
                report.push_error(
                    "UNKNOWN_TRIGGER_UNIT",
                    format!("Map '{map_id}' trigger '{tz_id}' spawns unknown unit_id '{unit_id}'"),
                );
            } else {
                check_unit_placement(unit_id, *tile, &format!("trigger '{tz_id}'"), report);
            }
        }
        TriggerAction::SpawnBuilding {
            building_id, tile, ..
        } => {
            if !loaded.buildings.contains_key(building_id) {
                report.push_error(
                    "UNKNOWN_TRIGGER_BUILDING",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' spawns unknown building_id '{building_id}'"
                    ),
                );
            } else if let Some(b_def) = loaded.buildings.get(building_id) {
                let fw = b_def.footprint[0].max(1) as i32;
                let fh = b_def.footprint[1].max(1) as i32;
                let max_t = [tile[0] + fw - 1, tile[1] + fh - 1];
                if !tile_in_bounds(*tile) || !tile_in_bounds(max_t) {
                    report.push_error(
                        "OUT_OF_BOUNDS_MAP_TILE",
                        format!(
                            "Map '{map_id}' trigger '{tz_id}' spawns building '{building_id}' out of bounds at {tile:?}"
                        ),
                    );
                }
            }
        }
        TriggerAction::ChangeOwner {
            building_id: Some(bid),
            ..
        } if !bid.is_empty() && !loaded.buildings.contains_key(bid) => {
            report.push_error(
                "UNKNOWN_TRIGGER_BUILDING",
                format!(
                    "Map '{map_id}' trigger '{tz_id}' ChangeOwner references unknown building_id '{bid}'"
                ),
            );
        }
        TriggerAction::FireSupportPower {
            power_id,
            target_tile,
            ..
        } => {
            if !defined_support_powers.contains(power_id.as_str()) {
                report.push_error(
                    "UNKNOWN_TRIGGER_SUPPORT_POWER",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' FireSupportPower references unknown support power '{power_id}'"
                    ),
                );
            }
            if !tile_in_bounds(*target_tile) {
                report.push_error(
                    "OUT_OF_BOUNDS_MAP_TILE",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' FireSupportPower target_tile {target_tile:?} is out of bounds"
                    ),
                );
            }
        }
        TriggerAction::RevealArea { tile, .. } | TriggerAction::FocusCamera { tile }
            if !tile_in_bounds(*tile) =>
        {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!("Map '{map_id}' trigger '{tz_id}' tile {tile:?} is out of bounds"),
            );
        }
        TriggerAction::UpsertObjective { objective, .. } => {
            if let Some(tile) = objective.marker_tile {
                if !tile_in_bounds(tile) {
                    report.push_error(
                        "OUT_OF_BOUNDS_MAP_TILE",
                        format!(
                            "Map '{map_id}' trigger '{tz_id}' UpsertObjective marker_tile {tile:?} is out of bounds"
                        ),
                    );
                }
            }
        }
        TriggerAction::SetObjectiveStatus { objective_id, .. }
        | TriggerAction::AddObjectiveProgress { objective_id, .. }
            if !objective_ids.is_empty() && !objective_ids.contains(objective_id.as_str()) =>
        {
            report.push_error(
                "UNKNOWN_OBJECTIVE",
                format!(
                    "Map '{map_id}' trigger '{tz_id}' references unknown objective_id '{objective_id}'"
                ),
            );
        }
        TriggerAction::PlayCutscene { cutscene_id }
            if !cutscene_ids.is_empty() && !cutscene_ids.contains(cutscene_id.as_str()) =>
        {
            report.push_error(
                "UNKNOWN_CUTSCENE",
                format!(
                    "Map '{map_id}' trigger '{tz_id}' PlayCutscene references unknown cutscene_id '{cutscene_id}'"
                ),
            );
        }
        TriggerAction::PlayRadarVideo { video_path } => {
            let full = loaded.resolve_asset_path(video_path);
            if !loaded.asset_exists(video_path) {
                report.push_error(
                    "MISSING_MISSION_MEDIA",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' PlayRadarVideo '{video_path}' does not exist at {}",
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    video_path,
                    &format!("Map '{map_id}' trigger '{tz_id}' PlayRadarVideo"),
                    checked_binary_assets,
                    report,
                );
            }
        }
        TriggerAction::EvaTransmission {
            portrait,
            audio_cue,
            radar_video,
            ..
        } => {
            validate_eva_transmission_media(
                map_id,
                tz_id,
                portrait.as_deref(),
                audio_cue.as_deref(),
                radar_video.as_deref(),
                loaded,
                checked_binary_assets,
                report,
            );
        }
        TriggerAction::ExpandPlayableBounds { min_tile, max_tile } => {
            if min_tile[0] > max_tile[0] || min_tile[1] > max_tile[1] {
                report.push_error(
                    "INVALID_MAP_RECT",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' ExpandPlayableBounds has inverted bounds"
                    ),
                );
            } else if !tile_in_bounds(*min_tile) || !tile_in_bounds(*max_tile) {
                report.push_error(
                    "OUT_OF_BOUNDS_MAP_RECT",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' ExpandPlayableBounds exceeds map bounds"
                    ),
                );
            }
        }
        TriggerAction::IssueOrder { target_tile, .. } if !tile_in_bounds(*target_tile) => {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' trigger '{tz_id}' IssueOrder target_tile {target_tile:?} is out of bounds"
                ),
            );
        }
        TriggerAction::ApplyStatusEffect { tile, status, .. } => {
            if let Some(t) = tile {
                if !tile_in_bounds(*t) {
                    report.push_error(
                        "OUT_OF_BOUNDS_MAP_TILE",
                        format!(
                            "Map '{map_id}' trigger '{tz_id}' ApplyStatusEffect tile {t:?} is out of bounds"
                        ),
                    );
                }
            }
            validate_status_effect_refs(
                status,
                &format!("Map '{map_id}' trigger '{tz_id}'"),
                loaded,
                report,
            );
        }
        TriggerAction::TeleportEntity {
            from_tile,
            destination_tile,
            ..
        } if !tile_in_bounds(*destination_tile)
            || from_tile.is_some_and(|ft| !tile_in_bounds(ft)) =>
        {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!("Map '{map_id}' trigger '{tz_id}' TeleportEntity tile is out of bounds"),
            );
        }
        TriggerAction::SpawnCaptureZone {
            zone_id,
            tile,
            radius,
            capture_time_secs,
            ..
        } => {
            if zone_id.trim().is_empty() || *radius <= 0.0 || *capture_time_secs <= 0.0 {
                report.push_error(
                    "INVALID_CAPTURE_ZONE",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' SpawnCaptureZone has invalid parameters"
                    ),
                );
            }
            if !tile_in_bounds(*tile) {
                report.push_error(
                    "OUT_OF_BOUNDS_MAP_TILE",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' SpawnCaptureZone tile {tile:?} is out of bounds"
                    ),
                );
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_eva_transmission_media(
    map_id: &str,
    tz_id: &str,
    portrait: Option<&str>,
    audio_cue: Option<&str>,
    radar_video: Option<&str>,
    loaded: &LoadedMod,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    for (label, opt_rel) in [("portrait", portrait), ("radar_video", radar_video)] {
        if let Some(rel) = opt_rel.filter(|s| !s.is_empty()) {
            let full = loaded.resolve_asset_path(rel);
            if !loaded.asset_exists(rel) {
                report.push_error(
                    "MISSING_MISSION_MEDIA",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' EvaTransmission {label} '{rel}' does not exist at {}",
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    rel,
                    &format!("Map '{map_id}' trigger '{tz_id}' {label}"),
                    checked_binary_assets,
                    report,
                );
            }
        }
    }
    if let Some(cue) = audio_cue.filter(|s| !s.is_empty()) {
        let looks_like_path = cue.contains('/')
            || cue.contains('\\')
            || cue.ends_with(".ogg")
            || cue.ends_with(".wav")
            || cue.ends_with(".flac");
        if looks_like_path {
            let full = loaded.resolve_asset_path(cue);
            if !loaded.asset_exists(cue) {
                report.push_error(
                    "MISSING_AUDIO_ASSET",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' EvaTransmission audio_cue '{cue}' does not exist at {}",
                        full.display()
                    ),
                );
            } else {
                check_existing_asset_binary(
                    loaded,
                    cue,
                    &format!("Map '{map_id}' trigger '{tz_id}' audio_cue"),
                    checked_binary_assets,
                    report,
                );
            }
        } else {
            let in_global = loaded.audio.eva_cues.contains_key(cue);
            let in_faction = loaded
                .audio
                .faction_eva_cues
                .values()
                .any(|m| m.contains_key(cue));
            if !in_global && !in_faction {
                report.push_error(
                    "MISSING_AUDIO_ASSET",
                    format!(
                        "Map '{map_id}' trigger '{tz_id}' EvaTransmission references unknown EVA cue '{cue}'"
                    ),
                );
            }
        }
    }
}
