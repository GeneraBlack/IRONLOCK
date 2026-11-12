use super::WorldBuilderTool;
use crate::rendering::{MapTerrainDirty, MapTerrainDirtyRects};
use crate::studio::history::{EditorCommand, TerrainStrokeState};
use crate::studio::spline_and_prefab_tools::blend_splat_rgba_toward_layer;
use crate::studio::StudioState;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    CivilianStructureDef, HeightmapDef, MapPropDef, PlayerSpawnDef, PropKind, TerrainSplatDef,
    TriggerZoneDef,
};
use rts_simulation::world::{MapSymmetryMode, SimCivilianStructure, SimWorld};

/// Applies the active `WorldBuilderTool` at `center_tile` (expanding across symmetry & brush radius)
/// and marks `terrain_dirty` + `MapTerrainDirtyRects` whenever terrain / splat / obstacle data changes.
#[allow(clippy::too_many_arguments)]
pub fn apply_world_builder_brush_at_tile(
    center_tile: [i32; 2],
    just_clicked: bool,
    placement_rotated: bool,
    state: &mut StudioState,
    loaded: &mut LoadedMod,
    world: &mut SimWorld,
    terrain_dirty: &mut MapTerrainDirty,
    mut terrain_dirty_rects: Option<&mut MapTerrainDirtyRects>,
) {
    if state.active_tool == WorldBuilderTool::SelectAndMove {
        return;
    }
    let w = world.grid.width;
    let h = world.grid.height;
    if center_tile[0] < 0 || center_tile[1] < 0 || center_tile[0] >= w || center_tile[1] >= h {
        return;
    }

    let sym_centers = expand_symmetry_centers(center_tile, w, h, state.symmetry);

    // Discrete placement tools fire once per LMB click (`just_clicked`)
    if !state.active_tool.is_continuous_terrain_brush() {
        if !just_clicked {
            return;
        }
        match state.active_tool {
            WorldBuilderTool::PlaceUnit => {
                let uid = if state.selected_unit_brush_id.is_empty() {
                    loaded.units.keys().next().cloned().unwrap_or_default()
                } else {
                    state.selected_unit_brush_id.clone()
                };
                if !uid.is_empty() {
                    for (idx, pt) in sym_centers.into_iter().enumerate() {
                        let pid = mirror_player_id(state.placement_owner_player, idx);
                        if let Some(eid) = world.spawn_unit(pid, &uid, pt, loaded) {
                            if placement_rotated {
                                if let Some(e) = world.entities.get_mut(&eid) {
                                    e.facing_radians = std::f32::consts::FRAC_PI_2;
                                    e.turret_facing_radians = std::f32::consts::FRAC_PI_2;
                                }
                            }
                            if let Some(ent) = world.entities.get(&eid).cloned() {
                                state.history.push_executed(EditorCommand::SpawnEntity {
                                    entity: Box::new(ent),
                                });
                            }
                        }
                    }
                }
            }
            WorldBuilderTool::PlaceBuilding => {
                let bid = if state.selected_building_brush_id.is_empty() {
                    loaded.buildings.keys().next().cloned().unwrap_or_default()
                } else {
                    state.selected_building_brush_id.clone()
                };
                if !bid.is_empty() {
                    for (idx, pt) in sym_centers.into_iter().enumerate() {
                        let pid = mirror_player_id(state.placement_owner_player, idx);
                        if let Some(eid) =
                            world.spawn_building(pid, &bid, pt, loaded, placement_rotated)
                        {
                            if let Some(ent) = world.entities.get(&eid).cloned() {
                                state.history.push_executed(EditorCommand::SpawnEntity {
                                    entity: Box::new(ent),
                                });
                            }
                        }
                    }
                }
            }
            WorldBuilderTool::PlaceCivilianStructure => {
                let before = TerrainStrokeState::capture(world, loaded);
                let fp = if placement_rotated {
                    [state.civ_footprint[1], state.civ_footprint[0]]
                } else {
                    state.civ_footprint
                };
                for pt in sym_centers {
                    let next_id = (world.civilian_structures.len() as u64) + 1;
                    let def = CivilianStructureDef {
                        id: format!("civ_bunker_{next_id}"),
                        tile_pos: pt,
                        footprint: fp,
                        max_health: state.civ_max_health,
                        cover_bonus: state.civ_cover_bonus,
                        leaves_ruin: state.civ_leaves_ruin,
                    };
                    let world_pos = world.grid.footprint_center_world(pt, fp);
                    world.civilian_structures.push(SimCivilianStructure {
                        id: next_id,
                        tile_pos: pt,
                        footprint: fp,
                        position: world_pos,
                        health: state.civ_max_health,
                        max_health: state.civ_max_health,
                        cover_bonus: state.civ_cover_bonus,
                        leaves_ruin: state.civ_leaves_ruin,
                        is_ruin: false,
                    });
                    world.grid.set_footprint_blocked(pt, fp, true);
                    loaded.entry_map.civilian_structures.push(def);
                    if let Some(rects) = terrain_dirty_rects.as_deref_mut() {
                        rects.mark_rect(pt, [pt[0] + fp[0] as i32, pt[1] + fp[1] as i32]);
                    }
                }
                world.grid.rebuild_clearance_map();
                let after = TerrainStrokeState::capture(world, loaded);
                state.history.push_executed(EditorCommand::TerrainStroke {
                    label: "Place Civilian Structure".to_string(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
                terrain_dirty.0 = true;
            }
            WorldBuilderTool::PlaceProp => {
                let before = TerrainStrokeState::capture(world, loaded);
                let kind = match state.selected_prop_kind_index {
                    0 => PropKind::Tree,
                    1 => PropKind::Streetlight,
                    _ => PropKind::ExplosiveBarrel,
                };
                for pt in sym_centers {
                    world.spawn_prop(kind, pt);
                    loaded
                        .entry_map
                        .props
                        .push(MapPropDef { kind, tile_pos: pt });
                    if let Some(rects) = terrain_dirty_rects.as_deref_mut() {
                        rects.mark_rect(pt, pt);
                    }
                }
                let after = TerrainStrokeState::capture(world, loaded);
                state.history.push_executed(EditorCommand::TerrainStroke {
                    label: "Place Prop".to_string(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
                terrain_dirty.0 = true;
            }
            WorldBuilderTool::PlacePlayerSpawn => {
                let before = TerrainStrokeState::capture(world, loaded);
                let default_fac = loaded
                    .factions
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "allies".to_string());
                for (idx, pt) in sym_centers.into_iter().enumerate() {
                    let pid = mirror_player_id(state.placement_owner_player.max(1), idx);
                    loaded.entry_map.spawns.retain(|s| s.player_id != pid);
                    loaded.entry_map.spawns.push(PlayerSpawnDef {
                        player_id: pid,
                        faction: default_fac.clone(),
                        sub_faction: None,
                        base_tile: pt,
                        team_id: pid,
                        initial_buildings: Vec::new(),
                        initial_units: Vec::new(),
                    });
                }
                let after = TerrainStrokeState::capture(world, loaded);
                state.history.push_executed(EditorCommand::TerrainStroke {
                    label: "Place Player Spawn".to_string(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
            }
            WorldBuilderTool::PlaceTriggerZone => {
                let before = TerrainStrokeState::capture(world, loaded);
                let r = state.brush_radius_tiles.max(1);
                let next_idx = loaded.entry_map.trigger_zones.len() + 1;
                let zone = TriggerZoneDef {
                    id: format!("zone_{next_idx}"),
                    min: [(center_tile[0] - r).max(0), (center_tile[1] - r).max(0)],
                    max: [
                        (center_tile[0] + r).min(w - 1),
                        (center_tile[1] + r).min(h - 1),
                    ],
                    once: true,
                    script_hook: String::new(),
                    condition: None,
                    actions: Vec::new(),
                };
                loaded.entry_map.trigger_zones.push(zone.clone());
                world.trigger_zones.push(zone);
                let after = TerrainStrokeState::capture(world, loaded);
                state.history.push_executed(EditorCommand::TerrainStroke {
                    label: "Create Trigger Zone".to_string(),
                    before: Box::new(before),
                    after: Box::new(after),
                });
            }
            _ => {}
        }
        return;
    }

    // Continuous sculpt / splat / tile brush
    let r = state.brush_radius_tiles.clamp(1, 8);
    let strength = state.brush_strength.clamp(0.05, 2.0);

    ensure_heightmap_and_splatmap(&mut loaded.entry_map);

    for sc in &sym_centers {
        if let Some(rects) = terrain_dirty_rects.as_deref_mut() {
            rects.mark_rect(
                [(sc[0] - r - 1).max(0), (sc[1] - r - 1).max(0)],
                [(sc[0] + r + 1).min(w - 1), (sc[1] + r + 1).min(h - 1)],
            );
        }
    }

    for sc in sym_centers {
        for dz in -r..=r {
            for dx in -r..=r {
                let tx = sc[0] + dx;
                let tz = sc[1] + dz;
                if tx < 0 || tz < 0 || tx >= w || tz >= h {
                    continue;
                }
                let dist = ((dx * dx + dz * dz) as f32).sqrt();
                if dist > r as f32 + 0.35 {
                    continue;
                }
                let falloff = (1.0 - (dist / (r as f32 + 0.5))).clamp(0.15, 1.0);

                match state.active_tool {
                    WorldBuilderTool::SculptRaise => {
                        sculpt_heightmap_tile(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            0.18 * strength * falloff,
                            None,
                        );
                        sync_tile_elevation_from_heightmap(&loaded.entry_map, world, [tx, tz]);
                    }
                    WorldBuilderTool::SculptLower => {
                        sculpt_heightmap_tile(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            -0.18 * strength * falloff,
                            None,
                        );
                        sync_tile_elevation_from_heightmap(&loaded.entry_map, world, [tx, tz]);
                    }
                    WorldBuilderTool::SculptFlatten => {
                        sculpt_heightmap_tile(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            0.35 * strength * falloff,
                            Some(state.flatten_target_height),
                        );
                        sync_tile_elevation_from_heightmap(&loaded.entry_map, world, [tx, tz]);
                    }
                    WorldBuilderTool::SculptSmooth => {
                        smooth_heightmap_tile(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            0.35 * strength * falloff,
                        );
                        sync_tile_elevation_from_heightmap(&loaded.entry_map, world, [tx, tz]);
                    }
                    WorldBuilderTool::SculptRamp => {
                        world.grid.ramp_tiles.insert([tx, tz]);
                        world.grid.elevation_tiles.remove(&[tx, tz]);
                        world.grid.blocked_tiles.remove(&[tx, tz]);
                    }
                    WorldBuilderTool::SplatGrass => {
                        paint_splat_layer(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            0,
                            strength * falloff,
                        );
                    }
                    WorldBuilderTool::SplatDirt => {
                        paint_splat_layer(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            1,
                            strength * falloff,
                        );
                    }
                    WorldBuilderTool::SplatSand => {
                        paint_splat_layer(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            2,
                            strength * falloff,
                        );
                    }
                    WorldBuilderTool::SplatAsphalt => {
                        paint_splat_layer(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            3,
                            strength * falloff,
                        );
                    }
                    WorldBuilderTool::SplatCliffRock => {
                        paint_splat_layer(
                            &mut loaded.entry_map,
                            tx as u32,
                            tz as u32,
                            4,
                            strength * falloff,
                        );
                    }
                    WorldBuilderTool::PaintWater => {
                        world.grid.water_tiles.insert([tx, tz]);
                        world.grid.ore_tiles.remove(&[tx, tz]);
                        world.grid.gem_tiles.remove(&[tx, tz]);
                    }
                    WorldBuilderTool::PaintBridge => {
                        world.grid.water_tiles.insert([tx, tz]);
                        world.grid.bridge_tiles.insert([tx, tz]);
                        world.grid.bridge_seed_tiles.insert([tx, tz]);
                    }
                    WorldBuilderTool::PaintRock => {
                        world.grid.blocked_tiles.insert([tx, tz]);
                    }
                    WorldBuilderTool::PaintOre => {
                        world.grid.ore_tiles.insert([tx, tz], 900);
                        if dx == 0 && dz == 0 {
                            world.grid.ore_seed_tiles.insert([tx, tz]);
                        }
                    }
                    WorldBuilderTool::PaintGems => {
                        world.grid.ore_tiles.insert([tx, tz], 1600);
                        world.grid.gem_tiles.insert([tx, tz]);
                    }
                    WorldBuilderTool::PaintCustomResource => {
                        let rid = if state.selected_custom_resource_id.is_empty() {
                            "timber".to_string()
                        } else {
                            state.selected_custom_resource_id.clone()
                        };
                        world
                            .resource_nodes
                            .entry(rid)
                            .or_default()
                            .insert([tx, tz], 1200);
                    }
                    WorldBuilderTool::ClearTerrain => {
                        world.grid.water_tiles.remove(&[tx, tz]);
                        world.grid.bridge_tiles.remove(&[tx, tz]);
                        world.grid.bridge_seed_tiles.remove(&[tx, tz]);
                        world.grid.blocked_tiles.remove(&[tx, tz]);
                        world.grid.elevation_tiles.remove(&[tx, tz]);
                        world.grid.ramp_tiles.remove(&[tx, tz]);
                        world.grid.ore_tiles.remove(&[tx, tz]);
                        world.grid.ore_seed_tiles.remove(&[tx, tz]);
                        world.grid.gem_tiles.remove(&[tx, tz]);
                        for nodes in world.resource_nodes.values_mut() {
                            nodes.remove(&[tx, tz]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    world.grid.sync_heightmap_from_map_def(&loaded.entry_map);
    loaded
        .maps
        .insert(loaded.entry_map.id.clone(), loaded.entry_map.clone());
    terrain_dirty.0 = true;
}

pub fn expand_symmetry_centers(
    c: [i32; 2],
    w: i32,
    h: i32,
    mode: MapSymmetryMode,
) -> Vec<[i32; 2]> {
    let mut out = vec![c];
    let push_uniq = |v: &mut Vec<[i32; 2]>, pt: [i32; 2]| {
        if pt[0] >= 0 && pt[1] >= 0 && pt[0] < w && pt[1] < h && !v.contains(&pt) {
            v.push(pt);
        }
    };
    match mode {
        MapSymmetryMode::Off => {}
        MapSymmetryMode::MirrorX => push_uniq(&mut out, [w - 1 - c[0], c[1]]),
        MapSymmetryMode::MirrorZ => push_uniq(&mut out, [c[0], h - 1 - c[1]]),
        MapSymmetryMode::Rotational2Way => push_uniq(&mut out, [w - 1 - c[0], h - 1 - c[1]]),
        MapSymmetryMode::Rotational4Way => {
            push_uniq(&mut out, [w - 1 - c[0], h - 1 - c[1]]);
            if w == h {
                push_uniq(&mut out, [h - 1 - c[1], c[0]]);
                push_uniq(&mut out, [c[1], w - 1 - c[0]]);
            } else {
                push_uniq(&mut out, [w - 1 - c[0], c[1]]);
                push_uniq(&mut out, [c[0], h - 1 - c[1]]);
            }
        }
    }
    out
}

fn mirror_player_id(base_pid: u8, sym_idx: usize) -> u8 {
    if base_pid == 0 || sym_idx == 0 {
        return base_pid;
    }
    ((base_pid - 1 + sym_idx as u8) % 8) + 1
}

pub fn ensure_heightmap_and_splatmap(map: &mut rts_sdk::schema::MapDef) {
    let vw = map.width + 1;
    let vh = map.height + 1;
    let expected_verts = (vw * vh) as usize;
    if map
        .heightmap
        .as_ref()
        .is_none_or(|hm| hm.heights.len() != expected_verts)
    {
        map.heightmap = Some(HeightmapDef {
            width: vw,
            height: vh,
            heights: vec![0.0; expected_verts],
        });
    }
    let canonical_layers = || {
        vec![
            "grass".to_string(),
            "dirt".to_string(),
            "sand".to_string(),
            "asphalt".to_string(),
        ]
    };
    match map.splatmap.as_mut() {
        Some(sp) => {
            if sp.layers.is_empty() || sp.layers.len() > 4 {
                sp.layers = canonical_layers();
            }
            if sp.weights_rgba.len() != expected_verts {
                let old_tile_len = (map.width * map.height) as usize;
                let mut migrated = vec![[255, 0, 0, 0]; expected_verts];
                if sp.weights_rgba.len() == old_tile_len && map.width > 0 && map.height > 0 {
                    for tz in 0..map.height {
                        for tx in 0..map.width {
                            migrated[(tz * vw + tx) as usize] =
                                sp.weights_rgba[(tz * map.width + tx) as usize];
                        }
                    }
                }
                sp.weights_rgba = migrated;
            }
        }
        None => {
            map.splatmap = Some(TerrainSplatDef {
                layers: canonical_layers(),
                weights_rgba: vec![[255, 0, 0, 0]; expected_verts],
            });
        }
    }
}

fn sculpt_heightmap_tile(
    map: &mut rts_sdk::schema::MapDef,
    tx: u32,
    tz: u32,
    delta: f32,
    flatten_to: Option<f32>,
) {
    let Some(hm) = map.heightmap.as_mut() else {
        return;
    };
    for oz in 0..=1 {
        for ox in 0..=1 {
            let vx = (tx + ox).min(hm.width.saturating_sub(1));
            let vz = (tz + oz).min(hm.height.saturating_sub(1));
            let idx = (vz * hm.width + vx) as usize;
            if let Some(h) = hm.heights.get_mut(idx) {
                if let Some(target) = flatten_to {
                    *h = (*h + (target - *h) * delta.clamp(0.05, 1.0)).clamp(-1.5, 6.0);
                } else {
                    *h = (*h + delta).clamp(-1.5, 6.0);
                }
            }
        }
    }
}

fn smooth_heightmap_tile(map: &mut rts_sdk::schema::MapDef, tx: u32, tz: u32, alpha: f32) {
    let Some(hm) = map.heightmap.as_mut() else {
        return;
    };
    let vx = tx.min(hm.width.saturating_sub(1));
    let vz = tz.min(hm.height.saturating_sub(1));
    let mut sum = 0.0f32;
    let mut count = 0.0f32;
    for dz in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = (vx as i32 + dx).clamp(0, hm.width as i32 - 1) as u32;
            let nz = (vz as i32 + dz).clamp(0, hm.height as i32 - 1) as u32;
            sum += hm.sample_vertex(nx, nz);
            count += 1.0;
        }
    }
    let avg = sum / count.max(1.0);
    let idx = (vz * hm.width + vx) as usize;
    if let Some(h) = hm.heights.get_mut(idx) {
        *h += (avg - *h) * alpha.clamp(0.05, 1.0);
    }
}

fn sync_tile_elevation_from_heightmap(
    map: &rts_sdk::schema::MapDef,
    world: &mut SimWorld,
    tile: [i32; 2],
) {
    let Some(hm) = map.heightmap.as_ref() else {
        return;
    };
    let h = hm.sample_vertex(tile[0] as u32, tile[1] as u32);
    if h >= 0.85 {
        world.grid.elevation_tiles.insert(tile);
    } else {
        world.grid.elevation_tiles.remove(&tile);
    }
}

fn paint_splat_layer(
    map: &mut rts_sdk::schema::MapDef,
    tx: u32,
    tz: u32,
    layer_idx: usize,
    strength: f32,
) {
    ensure_heightmap_and_splatmap(map);
    let vw = map.width + 1;
    let vh = map.height + 1;
    let Some(splat) = map.splatmap.as_mut() else {
        return;
    };
    let alpha = (strength.clamp(0.05, 1.5) * 0.65).clamp(0.05, 1.0);
    for oz in 0..=1 {
        for ox in 0..=1 {
            let vx = (tx + ox).min(vw.saturating_sub(1));
            let vz = (tz + oz).min(vh.saturating_sub(1));
            let idx = (vz * vw + vx) as usize;
            if let Some(rgba) = splat.weights_rgba.get_mut(idx) {
                blend_splat_rgba_toward_layer(rgba, layer_idx, alpha);
            }
        }
    }
}
