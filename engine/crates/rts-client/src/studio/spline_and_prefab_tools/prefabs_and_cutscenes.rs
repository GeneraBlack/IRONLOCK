use super::sync_grid_from_map_heightmap;
use crate::studio::world_builder::ensure_heightmap_and_splatmap;
use bevy::math::Vec3;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    CameraCutsceneDef, CameraKeyframe, MapDef, MapPrefabDef, MapPropDef, PreplacedEntityDef,
};
use rts_simulation::world::{EntityKind, SimWorld};

/// Rotates a relative tile offset `[rx, rz]` within a `[w, h]` footprint by `quarter_turns` (`0..=3` * 90° CW).
pub fn rotate_prefab_offset(offset: [i32; 2], size: [u32; 2], quarter_turns: u8) -> [i32; 2] {
    let w = size[0].max(1) as i32;
    let h = size[1].max(1) as i32;
    match quarter_turns % 4 {
        0 => offset,
        1 => [h - 1 - offset[1], offset[0]],
        2 => [w - 1 - offset[0], h - 1 - offset[1]],
        _ => [offset[1], w - 1 - offset[0]],
    }
}

pub fn rotated_prefab_footprint(size: [u32; 2], quarter_turns: u8) -> [u32; 2] {
    if quarter_turns % 2 == 1 {
        [size[1].max(1), size[0].max(1)]
    } else {
        [size[0].max(1), size[1].max(1)]
    }
}

/// Captures currently selected live entities, props, and underlying terrain height deltas & splat samples
/// into a reusable `MapPrefabDef`.
pub fn capture_selection_as_prefab(
    id: &str,
    display_name: &str,
    category: &str,
    selected_entity_ids: &std::collections::BTreeSet<u64>,
    selected_prop_ids: &std::collections::BTreeSet<u64>,
    map: &MapDef,
    world: &SimWorld,
) -> Option<MapPrefabDef> {
    let mut min_tx = i32::MAX;
    let mut min_tz = i32::MAX;
    let mut max_tx = i32::MIN;
    let mut max_tz = i32::MIN;

    for &eid in selected_entity_ids {
        if let Some(ent) = world.entities.get(&eid) {
            min_tx = min_tx.min(ent.tile_origin[0]);
            min_tz = min_tz.min(ent.tile_origin[1]);
            max_tx = max_tx.max(ent.tile_origin[0]);
            max_tz = max_tz.max(ent.tile_origin[1]);
        }
    }
    for &pid in selected_prop_ids {
        if let Some(prop) = world.props.iter().find(|p| p.id == pid) {
            min_tx = min_tx.min(prop.tile[0]);
            min_tz = min_tz.min(prop.tile[1]);
            max_tx = max_tx.max(prop.tile[0]);
            max_tz = max_tz.max(prop.tile[1]);
        }
    }

    if min_tx > max_tx || min_tz > max_tz {
        return None;
    }

    let fw = ((max_tx - min_tx) + 1).max(1) as u32;
    let fh = ((max_tz - min_tz) + 1).max(1) as u32;

    let mut entities = Vec::new();
    for &eid in selected_entity_ids {
        if let Some(ent) = world.entities.get(&eid) {
            let rel = [ent.tile_origin[0] - min_tx, ent.tile_origin[1] - min_tz];
            let tag = world.entity_tags.get(&eid).cloned();
            match &ent.kind {
                EntityKind::Unit { unit_id } => entities.push(PreplacedEntityDef {
                    player_id: ent.player_id,
                    building_id: None,
                    unit_id: Some(unit_id.clone()),
                    tile_pos: rel,
                    tag,
                }),
                EntityKind::Building { building_id } => entities.push(PreplacedEntityDef {
                    player_id: ent.player_id,
                    building_id: Some(building_id.clone()),
                    unit_id: None,
                    tile_pos: rel,
                    tag,
                }),
            }
        }
    }

    let mut props = Vec::new();
    for &pid in selected_prop_ids {
        if let Some(prop) = world.props.iter().find(|p| p.id == pid) {
            props.push(MapPropDef {
                kind: prop.kind,
                tile_pos: [prop.tile[0] - min_tx, prop.tile[1] - min_tz],
            });
        }
    }

    let base_h = map
        .heightmap
        .as_ref()
        .map(|hm| hm.sample_vertex(min_tx.max(0) as u32, min_tz.max(0) as u32))
        .unwrap_or(0.0);

    let mut height_deltas = Vec::with_capacity((fw * fh) as usize);
    let mut splat_samples = Vec::with_capacity((fw * fh) as usize);
    let vw = map.width + 1;
    for oz in 0..fh {
        for ox in 0..fw {
            let vx = (min_tx.max(0) as u32 + ox).min(map.width);
            let vz = (min_tz.max(0) as u32 + oz).min(map.height);
            let h = map
                .heightmap
                .as_ref()
                .map(|hm| hm.sample_vertex(vx, vz) - base_h)
                .unwrap_or(0.0);
            height_deltas.push(h);

            let rgba = map
                .splatmap
                .as_ref()
                .and_then(|sm| sm.weights_rgba.get((vz * vw + vx) as usize).copied())
                .unwrap_or([255, 0, 0, 0]);
            splat_samples.push(rgba);
        }
    }

    Some(MapPrefabDef {
        id: id.to_string(),
        display_name: display_name.to_string(),
        category: if category.trim().is_empty() {
            "general".to_string()
        } else {
            category.trim().to_string()
        },
        footprint_size: [fw, fh],
        height_deltas,
        splat_samples,
        entities,
        props,
    })
}

/// Stamps `prefab` at `anchor_tile` rotated by `quarter_turns * 90°`, spawning its entities/props
/// and applying its relative terrain height deltas & splat samples.
pub fn stamp_prefab_at_tile(
    prefab: &MapPrefabDef,
    anchor_tile: [i32; 2],
    quarter_turns: u8,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
) -> (usize, usize) {
    ensure_heightmap_and_splatmap(&mut loaded.entry_map);
    let orig_size = [
        prefab.footprint_size[0].max(1),
        prefab.footprint_size[1].max(1),
    ];
    let vw = loaded.entry_map.width + 1;
    let base_h = loaded
        .entry_map
        .heightmap
        .as_ref()
        .map(|hm| {
            hm.sample_vertex(
                anchor_tile[0].clamp(0, loaded.entry_map.width as i32) as u32,
                anchor_tile[1].clamp(0, loaded.entry_map.height as i32) as u32,
            )
        })
        .unwrap_or(0.0);

    for oz in 0..orig_size[1] {
        for ox in 0..orig_size[0] {
            let src_idx = (oz * orig_size[0] + ox) as usize;
            let rot_off = rotate_prefab_offset([ox as i32, oz as i32], orig_size, quarter_turns);
            let tx = anchor_tile[0] + rot_off[0];
            let tz = anchor_tile[1] + rot_off[1];
            if tx < 0
                || tz < 0
                || tx >= loaded.entry_map.width as i32
                || tz >= loaded.entry_map.height as i32
            {
                continue;
            }
            let vx = tx as u32;
            let vz = tz as u32;
            let v_idx = (vz * vw + vx) as usize;
            if let Some(&dh) = prefab.height_deltas.get(src_idx) {
                if let Some(hm) = loaded.entry_map.heightmap.as_mut() {
                    if let Some(slot) = hm.heights.get_mut(v_idx) {
                        *slot = (base_h + dh).clamp(-1.5, 6.0);
                    }
                }
            }
            if let Some(&rgba) = prefab.splat_samples.get(src_idx) {
                if let Some(sm) = loaded.entry_map.splatmap.as_mut() {
                    if let Some(slot) = sm.weights_rgba.get_mut(v_idx) {
                        *slot = rgba;
                    }
                }
            }
        }
    }
    sync_grid_from_map_heightmap(&loaded.entry_map, world);

    let mut spawned_ents = 0usize;
    for pe in &prefab.entities {
        let rot_off = rotate_prefab_offset(pe.tile_pos, orig_size, quarter_turns);
        let dest = [anchor_tile[0] + rot_off[0], anchor_tile[1] + rot_off[1]];
        if !world.grid.in_bounds(dest) {
            continue;
        }
        let spawned_id = if let Some(bid) = pe.building_id.as_deref().filter(|s| !s.is_empty()) {
            world.spawn_building(pe.player_id, bid, dest, loaded, false)
        } else if let Some(uid) = pe.unit_id.as_deref().filter(|s| !s.is_empty()) {
            world.spawn_unit(pe.player_id, uid, dest, loaded)
        } else {
            None
        };
        if let Some(eid) = spawned_id {
            spawned_ents += 1;
            if let Some(tag) = pe.tag.as_ref().filter(|t| !t.is_empty()) {
                world.entity_tags.insert(eid, tag.clone());
            }
        }
    }

    let mut spawned_props = 0usize;
    for pr in &prefab.props {
        let rot_off = rotate_prefab_offset(pr.tile_pos, orig_size, quarter_turns);
        let dest = [anchor_tile[0] + rot_off[0], anchor_tile[1] + rot_off[1]];
        if world.grid.in_bounds(dest) {
            world.spawn_prop(pr.kind, dest);
            spawned_props += 1;
        }
    }

    world.recalculate_power(loaded);
    (spawned_ents, spawned_props)
}

/// Evaluates `cutscene` at `time_secs` (`Some((focus, distance, pitch_deg, yaw_deg))`),
/// returning `None` once `time_secs` exceeds the final keyframe timestamp.
pub fn sample_cutscene_camera_pose(
    cutscene: &CameraCutsceneDef,
    time_secs: f32,
) -> Option<(Vec3, f32, f32, f32)> {
    if cutscene.keyframes.is_empty() {
        return None;
    }
    if cutscene.keyframes.len() == 1 {
        let kf = &cutscene.keyframes[0];
        if time_secs > kf.time_secs.max(2.0) {
            return None;
        }
        return Some((Vec3::from(kf.focus), kf.distance, kf.pitch_deg, kf.yaw_deg));
    }
    let last_t = cutscene
        .keyframes
        .last()
        .map(|k| k.time_secs)
        .unwrap_or(0.0);
    if time_secs > last_t {
        return None;
    }
    for pair in cutscene.keyframes.windows(2) {
        let a: &CameraKeyframe = &pair[0];
        let b: &CameraKeyframe = &pair[1];
        if time_secs <= b.time_secs {
            let span = (b.time_secs - a.time_secs).max(0.001);
            let u = ((time_secs - a.time_secs) / span).clamp(0.0, 1.0);
            let smooth = u * u * (3.0 - 2.0 * u);
            let focus = Vec3::from(a.focus).lerp(Vec3::from(b.focus), smooth);
            let dist = a.distance + (b.distance - a.distance) * smooth;
            let pitch = a.pitch_deg + (b.pitch_deg - a.pitch_deg) * smooth;
            let yaw = a.yaw_deg + (b.yaw_deg - a.yaw_deg) * smooth;
            return Some((focus, dist, pitch, yaw));
        }
    }
    let last = cutscene.keyframes.last()?;
    Some((
        Vec3::from(last.focus),
        last.distance,
        last.pitch_deg,
        last.yaw_deg,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::spline_and_prefab_tools::{
        apply_auto_ramp_grading, rasterize_spline_into_map_and_world, SplineKind,
    };
    use rts_asset_pipeline::scaffold::scaffold_new_mod_project;
    use rts_sdk::validator::validate_mod;
    use std::collections::BTreeSet;

    #[test]
    fn spline_rasterization_auto_ramp_and_prefab_stamp_preserve_valid_splat_stride() {
        let tmp = std::env::temp_dir().join(format!(
            "rts_studio_spline_prefab_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        scaffold_new_mod_project(&tmp, "spline-test", "Spline Test", None).expect("scaffold");

        let mut loaded = LoadedMod::load_from_dir(&tmp).expect("load");
        let mut world = SimWorld::from_mod(&loaded);

        // 1. Catmull-Rom River & CliffRidge splines
        let river_pts = vec![[10.0, 0.0, 10.0], [20.0, 0.0, 14.0], [32.0, 0.0, 18.0]];
        let touched = rasterize_spline_into_map_and_world(
            &river_pts,
            SplineKind::River,
            1.5,
            0.9,
            &mut loaded.entry_map,
            &mut world,
        );
        assert!(touched > 10);
        assert!(!world.grid.water_tiles.is_empty());

        let ridge_pts = vec![[12.0, 1.5, 28.0], [24.0, 1.5, 30.0]];
        let touched_ridge = rasterize_spline_into_map_and_world(
            &ridge_pts,
            SplineKind::CliffRidge,
            1.2,
            2.0,
            &mut loaded.entry_map,
            &mut world,
        );
        assert!(touched_ridge > 5);
        assert!(!world.grid.blocked_tiles.is_empty());

        // Verify splatmap vertex stride is strictly (w+1)*(h+1) and CliffRock uses [0,0,0,0]
        let expected_v = ((loaded.entry_map.width + 1) * (loaded.entry_map.height + 1)) as usize;
        let sm = loaded.entry_map.splatmap.as_ref().expect("splatmap");
        assert_eq!(sm.weights_rgba.len(), expected_v);
        assert!(
            sm.weights_rgba.contains(&[0, 0, 0, 0]),
            "CliffRidge spline must write [0,0,0,0] CliffRock weights"
        );

        // 2. Auto-Ramp Grading
        let ramp_tiles =
            apply_auto_ramp_grading([6, 20], [12, 20], 1.5, &mut loaded.entry_map, &mut world);
        assert!(ramp_tiles >= 5);
        assert!(world.grid.ramp_tiles.contains(&[9, 20]));

        // 3. Capture & Stamp Prefab with 90-degree rotation
        let first_uid = loaded.units.keys().next().expect("unit").clone();
        let e1 = world
            .spawn_unit(1, &first_uid, [14, 14], &loaded)
            .expect("spawn");
        world.entity_tags.insert(e1, "convoy_lead".to_string());
        let p1 = world.spawn_prop(rts_sdk::schema::PropKind::Tree, [16, 15]);
        let mut sel_ents = BTreeSet::new();
        sel_ents.insert(e1);
        let mut sel_props = BTreeSet::new();
        sel_props.insert(p1);

        let prefab = capture_selection_as_prefab(
            "convoy_camp",
            "Convoy Camp",
            "military",
            &sel_ents,
            &sel_props,
            &loaded.entry_map,
            &world,
        )
        .expect("capture prefab");
        assert_eq!(prefab.footprint_size, [3, 2]);
        loaded.save_prefab_def(&prefab).expect("save prefab");
        assert_eq!(loaded.load_prefab_defs().expect("load prefabs").len(), 1);

        let ents_before = world.entities.len();
        let props_before = world.props.len();
        let (se, sp) = stamp_prefab_at_tile(&prefab, [28, 28], 1, &mut loaded, &mut world);
        assert_eq!(se, 1);
        assert_eq!(sp, 1);
        assert_eq!(world.entities.len(), ents_before + 1);
        assert_eq!(world.props.len(), props_before + 1);

        // Validate map + mod has 0 validation errors
        loaded
            .maps
            .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
        let rep = validate_mod(&loaded, false);
        assert_eq!(
            rep.error_count(),
            0,
            "Expected 0 validation errors after spline/ramp/prefab ops, got: {:?}",
            rep.issues
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
