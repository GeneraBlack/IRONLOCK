use super::trigger_graph_check::validate_trigger_graph;
use super::triggers_check::{validate_trigger_action_on_map, validate_trigger_condition};
use super::ValidationReport;
use crate::loader::LoadedMod;
use crate::schema::TriggerAction;
use std::collections::BTreeSet;

pub(super) fn validate_map_def(
    map_def: &crate::schema::MapDef,
    loaded: &LoadedMod,
    defined_support_powers: &BTreeSet<&str>,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    let map_id = map_def.id.as_str();
    if map_def.width == 0 || map_def.height == 0 {
        report.push_error(
            "INVALID_MAP_DIMENSIONS",
            format!(
                "Map '{map_id}' has invalid zero dimensions {}x{}",
                map_def.width, map_def.height
            ),
        );
        return;
    }

    let w = map_def.width as i32;
    let h = map_def.height as i32;
    let tile_in_bounds =
        |tile: [i32; 2]| tile[0] >= 0 && tile[0] < w && tile[1] >= 0 && tile[1] < h;

    let check_rect = |kind: &str,
                      min: [i32; 2],
                      max: [i32; 2],
                      report: &mut ValidationReport|
     -> bool {
        if min[0] > max[0] || min[1] > max[1] {
            report.push_error(
                "INVALID_MAP_RECT",
                format!("Map '{map_id}' {kind} has inverted bounds min={min:?} > max={max:?}"),
            );
            return false;
        }
        if !tile_in_bounds(min) || !tile_in_bounds(max) {
            report.push_error(
                    "OUT_OF_BOUNDS_MAP_RECT",
                    format!(
                        "Map '{map_id}' {kind} rect {min:?}..={max:?} exceeds map bounds 0..{w} x 0..{h}"
                    ),
                );
            return false;
        }
        true
    };

    if let Some((pb_min, pb_max)) = map_def.playable_bounds {
        let _ = check_rect("playable_bounds", pb_min, pb_max, report);
    }

    let mut objective_ids: BTreeSet<&str> = BTreeSet::new();
    for obj in &map_def.objectives {
        if obj.id.trim().is_empty() {
            report.push_error(
                "INVALID_OBJECTIVE",
                format!("Map '{map_id}' defines an objective with empty id"),
            );
        } else if !objective_ids.insert(obj.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!("Map '{map_id}' defines duplicate objective id '{}'", obj.id),
            );
        }
        if let Some(tile) = obj.marker_tile {
            if !tile_in_bounds(tile) {
                report.push_error(
                    "OUT_OF_BOUNDS_MAP_TILE",
                    format!(
                        "Map '{map_id}' objective '{}' marker_tile {tile:?} is out of bounds",
                        obj.id
                    ),
                );
            }
        }
    }
    for tz in &map_def.trigger_zones {
        for act in &tz.actions {
            if let TriggerAction::UpsertObjective { objective, .. } = act {
                if !objective.id.trim().is_empty() {
                    objective_ids.insert(objective.id.as_str());
                }
            }
        }
    }

    let mut cutscene_ids: BTreeSet<&str> = BTreeSet::new();
    for cs in &map_def.cutscenes {
        if cs.id.trim().is_empty() {
            report.push_error(
                "INVALID_CUTSCENE",
                format!("Map '{map_id}' defines a cutscene with empty id"),
            );
        } else if !cutscene_ids.insert(cs.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!("Map '{map_id}' defines duplicate cutscene id '{}'", cs.id),
            );
        }
        for (kf_idx, kf) in cs.keyframes.iter().enumerate() {
            if !kf.time_secs.is_finite()
                || kf.time_secs < 0.0
                || !kf.distance.is_finite()
                || kf.distance <= 0.0
                || kf.focus.iter().any(|c| !c.is_finite())
                || !kf.pitch_deg.is_finite()
                || !kf.yaw_deg.is_finite()
            {
                report.push_error(
                    "INVALID_CUTSCENE",
                    format!(
                        "Map '{map_id}' cutscene '{}' keyframe[{kf_idx}] has invalid parameters",
                        cs.id
                    ),
                );
            }
        }
    }

    let zone_ids: BTreeSet<&str> = map_def
        .trigger_zones
        .iter()
        .map(|z| z.id.as_str())
        .chain(map_def.capture_zones.iter().map(|cz| cz.id.as_str()))
        .collect();

    let mut water_tiles: BTreeSet<[i32; 2]> = BTreeSet::new();
    let mut bridge_tiles: BTreeSet<[i32; 2]> = BTreeSet::new();
    let mut obstacle_tiles: BTreeSet<[i32; 2]> = BTreeSet::new();

    for obs in &map_def.obstacles {
        if check_rect("obstacle", obs.min, obs.max, report) {
            for tz in obs.min[1]..=obs.max[1] {
                for tx in obs.min[0]..=obs.max[0] {
                    obstacle_tiles.insert([tx, tz]);
                }
            }
        }
    }
    for wr in &map_def.water_rects {
        if check_rect("water_rect", wr.min, wr.max, report) {
            for tz in wr.min[1]..=wr.max[1] {
                for tx in wr.min[0]..=wr.max[0] {
                    water_tiles.insert([tx, tz]);
                }
            }
        }
    }
    for br in &map_def.bridge_rects {
        if check_rect("bridge_rect", br.min, br.max, report) {
            for tz in br.min[1]..=br.max[1] {
                for tx in br.min[0]..=br.max[0] {
                    bridge_tiles.insert([tx, tz]);
                    obstacle_tiles.remove(&[tx, tz]);
                }
            }
        }
    }
    for er in &map_def.elevation_rects {
        let _ = check_rect("elevation_rect", er.min, er.max, report);
    }
    for rr in &map_def.ramp_rects {
        let _ = check_rect("ramp_rect", rr.min, rr.max, report);
    }

    for patch in &map_def.ore_patches {
        if !tile_in_bounds(patch.position) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' ore_patch center {:?} is outside 0..{w} x 0..{h}",
                    patch.position
                ),
            );
        }
    }
    for patch in &map_def.gem_patches {
        if !tile_in_bounds(patch.position) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' gem_patch center {:?} is outside 0..{w} x 0..{h}",
                    patch.position
                ),
            );
        }
    }
    for prop in &map_def.props {
        if !tile_in_bounds(prop.tile_pos) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' prop {:?} at {:?} is outside 0..{w} x 0..{h}",
                    prop.kind, prop.tile_pos
                ),
            );
        }
    }

    // Validate Phase 25 capture_zones & item_drops
    let mut seen_cz_ids = BTreeSet::new();
    for cz in &map_def.capture_zones {
        if cz.id.trim().is_empty()
            || !cz.radius.is_finite()
            || cz.radius <= 0.0
            || !cz.capture_time_secs.is_finite()
            || cz.capture_time_secs <= 0.0
        {
            report.push_error(
                "INVALID_CAPTURE_ZONE",
                format!(
                    "Map '{map_id}' capture_zone '{}' has invalid radius or capture_time_secs",
                    cz.id
                ),
            );
        } else if !seen_cz_ids.insert(cz.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!(
                    "Map '{map_id}' defines duplicate capture_zone id '{}'",
                    cz.id
                ),
            );
        }
        if !tile_in_bounds(cz.tile_pos) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' capture_zone '{}' tile_pos {:?} is out of bounds",
                    cz.id, cz.tile_pos
                ),
            );
        }
    }
    for drop in &map_def.item_drops {
        if !tile_in_bounds(drop.tile_pos) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' item_drop '{}' tile_pos {:?} is out of bounds",
                    drop.item_id, drop.tile_pos
                ),
            );
        }
        if drop.item_id.trim().is_empty() || !loaded.items.contains_key(&drop.item_id) {
            report.push_error(
                "UNKNOWN_ITEM",
                format!(
                    "Map '{map_id}' item_drops references unknown item_id '{}'",
                    drop.item_id
                ),
            );
        }
    }

    let check_building_placement =
        |b_id: &str, tile_pos: [i32; 2], ctx_label: &str, report: &mut ValidationReport| {
            let Some(b_def) = loaded.buildings.get(b_id) else {
                return;
            };
            let fw = b_def.footprint[0].max(1) as i32;
            let fh = b_def.footprint[1].max(1) as i32;
            let max_tile = [tile_pos[0] + fw - 1, tile_pos[1] + fh - 1];
            if !tile_in_bounds(tile_pos) || !tile_in_bounds(max_tile) {
                report.push_error(
                    "OUT_OF_BOUNDS_MAP_TILE",
                    format!(
                        "Map '{map_id}' {ctx_label} building '{b_id}' footprint {tile_pos:?}..={max_tile:?} exceeds map bounds 0..{w} x 0..{h}"
                    ),
                );
                return;
            }
            for tz in tile_pos[1]..=max_tile[1] {
                for tx in tile_pos[0]..=max_tile[0] {
                    let t = [tx, tz];
                    if b_def.water_bound {
                        if !water_tiles.contains(&t) {
                            report.push_error(
                                "WATER_BUILDING_ON_DRY_LAND",
                                format!(
                                    "Map '{map_id}' {ctx_label} water-bound building '{b_id}' at {tile_pos:?} covers dry tile {t:?}"
                                ),
                            );
                            return;
                        }
                    } else if b_def.role != crate::schema::BuildingRole::Bridge {
                        if water_tiles.contains(&t) && !bridge_tiles.contains(&t) {
                            report.push_error(
                                "BUILDING_ON_WATER",
                                format!(
                                    "Map '{map_id}' {ctx_label} land building '{b_id}' at {tile_pos:?} overlaps unbridged water tile {t:?}"
                                ),
                            );
                            return;
                        }
                        if obstacle_tiles.contains(&t) {
                            report.push_error(
                                "BUILDING_ON_OBSTACLE",
                                format!(
                                    "Map '{map_id}' {ctx_label} building '{b_id}' at {tile_pos:?} overlaps obstacle tile {t:?}"
                                ),
                            );
                            return;
                        }
                    }
                }
            }
        };

    let check_unit_placement = |u_id: &str,
                                tile_pos: [i32; 2],
                                ctx_label: &str,
                                report: &mut ValidationReport| {
        let Some(u_def) = loaded.units.get(u_id) else {
            return;
        };
        if !tile_in_bounds(tile_pos) {
            report.push_error(
                    "OUT_OF_BOUNDS_MAP_TILE",
                    format!(
                        "Map '{map_id}' {ctx_label} unit '{u_id}' at {tile_pos:?} is outside 0..{w} x 0..{h}"
                    ),
                );
            return;
        }
        match u_def.movement_domain {
            crate::schema::MovementDomain::Naval if !water_tiles.contains(&tile_pos) => {
                report.push_error(
                        "NAVAL_UNIT_ON_DRY_LAND",
                        format!(
                            "Map '{map_id}' {ctx_label} naval unit '{u_id}' at {tile_pos:?} is placed on dry land (outside water_rects)"
                        ),
                    );
            }
            crate::schema::MovementDomain::Ground => {
                if water_tiles.contains(&tile_pos) && !bridge_tiles.contains(&tile_pos) {
                    report.push_error(
                            "GROUND_UNIT_ON_WATER",
                            format!(
                                "Map '{map_id}' {ctx_label} ground unit '{u_id}' at {tile_pos:?} is placed on unbridged water"
                            ),
                        );
                } else if obstacle_tiles.contains(&tile_pos) {
                    report.push_error(
                            "UNIT_ON_OBSTACLE",
                            format!(
                                "Map '{map_id}' {ctx_label} ground unit '{u_id}' at {tile_pos:?} is placed inside an obstacle"
                            ),
                        );
                }
            }
            _ => {}
        }
    };

    for spawn in &map_def.spawns {
        if !loaded.factions.contains_key(&spawn.faction) {
            report.push_error(
                "UNKNOWN_MAP_SPAWN_FACTION",
                format!(
                    "Map '{map_id}' player {} references unknown faction '{}'",
                    spawn.player_id, spawn.faction
                ),
            );
        }
        if !tile_in_bounds(spawn.base_tile) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' player {} base_tile {:?} is outside 0..{w} x 0..{h}",
                    spawn.player_id, spawn.base_tile
                ),
            );
        }
        let sp_ctx = format!("spawn(player={})", spawn.player_id);
        for ib in &spawn.initial_buildings {
            if !loaded.buildings.contains_key(&ib.building_id) {
                report.push_error(
                    "UNKNOWN_MAP_SPAWN_BUILDING",
                    format!(
                        "Map '{map_id}' spawns unknown building '{}'",
                        ib.building_id
                    ),
                );
            } else {
                check_building_placement(&ib.building_id, ib.tile_pos, &sp_ctx, report);
            }
        }
        for iu in &spawn.initial_units {
            if !loaded.units.contains_key(&iu.unit_id) {
                report.push_error(
                    "UNKNOWN_MAP_SPAWN_UNIT",
                    format!("Map '{map_id}' spawns unknown unit '{}'", iu.unit_id),
                );
            } else {
                check_unit_placement(&iu.unit_id, iu.tile_pos, &sp_ctx, report);
            }
        }
    }

    for (idx, pe) in map_def.preplaced_entities.iter().enumerate() {
        let pe_ctx = format!("preplaced_entities[{idx}](player={})", pe.player_id);
        let has_b = pe.building_id.as_deref().is_some_and(|s| !s.is_empty());
        let has_u = pe.unit_id.as_deref().is_some_and(|s| !s.is_empty());
        if !has_b && !has_u {
            report.push_error(
                "EMPTY_PREPLACED_ENTITY",
                format!(
                    "Map '{map_id}' {pe_ctx} at {:?} specifies neither building_id nor unit_id",
                    pe.tile_pos
                ),
            );
            continue;
        }
        if let Some(bid) = pe.building_id.as_deref().filter(|s| !s.is_empty()) {
            if !loaded.buildings.contains_key(bid) {
                report.push_error(
                    "UNKNOWN_MAP_BUILDING",
                    format!("Map '{map_id}' {pe_ctx} references unknown building_id '{bid}'"),
                );
            } else {
                check_building_placement(bid, pe.tile_pos, &pe_ctx, report);
            }
        }
        if let Some(uid) = pe.unit_id.as_deref().filter(|s| !s.is_empty()) {
            if !loaded.units.contains_key(uid) {
                report.push_error(
                    "UNKNOWN_MAP_UNIT",
                    format!("Map '{map_id}' {pe_ctx} references unknown unit_id '{uid}'"),
                );
            } else {
                check_unit_placement(uid, pe.tile_pos, &pe_ctx, report);
            }
        }
    }

    for tz in &map_def.trigger_zones {
        if !tz.script_hook.is_empty() && !loaded.script_host.has_hook(&tz.script_hook) {
            report.push_error(
                "UNKNOWN_SCRIPT_HOOK",
                format!(
                    "Map '{map_id}' trigger '{}' references undefined script_hook '{}'",
                    tz.id, tz.script_hook
                ),
            );
        }
        let mut uses_zone = tz.condition.is_none();
        if let Some(cond) = &tz.condition {
            if validate_trigger_condition(
                cond,
                map_id,
                &tz.id,
                &zone_ids,
                &objective_ids,
                loaded,
                report,
            ) {
                uses_zone = true;
            }
        }
        if uses_zone || tz.min != [0, 0] || tz.max != [0, 0] {
            let _ = check_rect(&format!("trigger '{}'", tz.id), tz.min, tz.max, report);
        }
        for act in &tz.actions {
            validate_trigger_action_on_map(
                act,
                map_id,
                &tz.id,
                w,
                h,
                &objective_ids,
                &cutscene_ids,
                defined_support_powers,
                loaded,
                &check_unit_placement,
                checked_binary_assets,
                report,
            );
        }
    }

    for graph in &map_def.trigger_graphs {
        validate_trigger_graph(
            graph,
            &format!("Map '{map_id}'"),
            Some(map_def),
            &zone_ids,
            &objective_ids,
            &cutscene_ids,
            defined_support_powers,
            loaded,
            checked_binary_assets,
            report,
        );
    }

    validate_terrain_civilian_and_resource_patches(map_def, loaded, report);
}

fn validate_terrain_civilian_and_resource_patches(
    map_def: &crate::schema::MapDef,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) {
    let map_id = map_def.id.as_str();
    let w = map_def.width as i32;
    let h = map_def.height as i32;
    let tile_in_bounds =
        |tile: [i32; 2]| tile[0] >= 0 && tile[0] < w && tile[1] >= 0 && tile[1] < h;

    let expected_vw = map_def.width + 1;
    let expected_vh = map_def.height + 1;
    let expected_vertex_count = (expected_vw as usize).saturating_mul(expected_vh as usize);
    if let Some(hm) = &map_def.heightmap {
        if hm.width != expected_vw
            || hm.height != expected_vh
            || hm.heights.len() != expected_vertex_count
            || hm.heights.iter().any(|h_val| !h_val.is_finite())
        {
            report.push_error(
                "INVALID_HEIGHTMAP",
                format!(
                    "Map '{map_id}' heightmap must have dimensions {}x{} ({} finite floats), got {}x{} ({} entries)",
                    expected_vw,
                    expected_vh,
                    expected_vertex_count,
                    hm.width,
                    hm.height,
                    hm.heights.len()
                ),
            );
        }
    }
    if let Some(sm) = &map_def.splatmap {
        let expected_splat_bytes = expected_vertex_count.saturating_mul(4);
        let actual_splat_bytes = sm.weights_rgba.len().saturating_mul(4);
        if sm.layers.is_empty()
            || sm.layers.len() > 4
            || sm.layers.iter().any(|l| l.trim().is_empty())
            || sm.weights_rgba.len() != expected_vertex_count
            || actual_splat_bytes != expected_splat_bytes
        {
            report.push_error(
                "INVALID_SPLATMAP",
                format!(
                    "Map '{map_id}' splatmap must define 1..=4 non-empty layers (got {}) and {} RGBA vertex weights ({} bytes, got {} weights / {} bytes)",
                    sm.layers.len(),
                    expected_vertex_count,
                    expected_splat_bytes,
                    sm.weights_rgba.len(),
                    actual_splat_bytes
                ),
            );
        }
    }

    let mut seen_civ_ids = BTreeSet::new();
    for civ in &map_def.civilian_structures {
        if civ.id.trim().is_empty() {
            report.push_error(
                "INVALID_CIVILIAN_STRUCTURE",
                format!("Map '{map_id}' defines a civilian_structure with an empty id"),
            );
        } else if !seen_civ_ids.insert(civ.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!(
                    "Map '{map_id}' defines duplicate civilian_structure id '{}'",
                    civ.id
                ),
            );
        }
        let fw = civ.footprint[0];
        let fh = civ.footprint[1];
        if fw == 0
            || fh == 0
            || !civ.max_health.is_finite()
            || civ.max_health <= 0.0
            || !civ.cover_bonus.is_finite()
            || !(0.0..=1.0).contains(&civ.cover_bonus)
        {
            report.push_error(
                "INVALID_CIVILIAN_STRUCTURE",
                format!(
                    "Map '{map_id}' civilian_structure '{}' has invalid footprint {:?}, max_health {}, or cover_bonus {}",
                    civ.id, civ.footprint, civ.max_health, civ.cover_bonus
                ),
            );
            continue;
        }
        let max_tile = [
            civ.tile_pos[0] + (fw as i32) - 1,
            civ.tile_pos[1] + (fh as i32) - 1,
        ];
        if !tile_in_bounds(civ.tile_pos) || !tile_in_bounds(max_tile) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' civilian_structure '{}' footprint {:?}..={max_tile:?} exceeds map bounds 0..{w} x 0..{h}",
                    civ.id, civ.tile_pos
                ),
            );
        }
    }

    for patch in &map_def.resource_patches {
        if !tile_in_bounds(patch.position) {
            report.push_error(
                "OUT_OF_BOUNDS_MAP_TILE",
                format!(
                    "Map '{map_id}' resource_patch ('{}') center {:?} is outside 0..{w} x 0..{h}",
                    patch.resource_id, patch.position
                ),
            );
        }
        if !loaded.resources.contains_key(&patch.resource_id) {
            report.push_error(
                "UNKNOWN_RESOURCE_TYPE",
                format!(
                    "Map '{map_id}' resource_patch references unknown resource_id '{}'",
                    patch.resource_id
                ),
            );
        }
        if patch.amount_per_tile == 0 {
            report.push_error(
                "INVALID_RESOURCE_PATCH",
                format!(
                    "Map '{map_id}' resource_patch ('{}') must have amount_per_tile > 0",
                    patch.resource_id
                ),
            );
        }
    }
}
