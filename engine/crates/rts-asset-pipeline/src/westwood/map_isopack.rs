use super::WestwoodIni;
use anyhow::Result;
use rts_sdk::schema::{
    FactionCatalog, HeightmapDef, MapDef, MapPropDef, PropKind, TerrainSplatDef, TriggerAction,
    UnitDef,
};
use std::collections::{BTreeMap, BTreeSet};

use super::map_pack_codec::cluster_resource_tiles_into_patches;
pub use super::map_pack_codec::{
    decode_ini_section_base64, decompress_lzo1x, decompress_westwood_pack_chunks,
    encode_isomappack5_records, encode_westwood_base64_ini_lines,
    encode_westwood_format80_pack_ini_lines, merge_tiles_into_rects, parse_isomappack5_records,
    parse_westwood_waypoint_token, DecodedIsoMapPack, IsoMapTileRecord,
};
pub use super::map_triggers::parse_westwood_map_triggers;

fn map_iso_coord_to_grid(
    rx: i16,
    ry: i16,
    direct_grid_mode: bool,
    iso_bounds: (i32, i32, i32, i32),
    map_raw_w: i32,
    width: u32,
    height: u32,
) -> Option<[i32; 2]> {
    if direct_grid_mode {
        if rx >= 0 && ry >= 0 && (rx as u32) < width && (ry as u32) < height {
            return Some([rx as i32, ry as i32]);
        }
        return None;
    }

    let dx = (rx as i32) - (ry as i32) + map_raw_w - 1;
    let dy = (rx as i32) + (ry as i32) - map_raw_w - 1;
    let (min_dx, max_dx, min_dy, max_dy) = iso_bounds;
    let span_x = (max_dx - min_dx).max(1);
    let span_y = (max_dy - min_dy).max(1);
    let tx = (((dx - min_dx) as i64 * (width.saturating_sub(1) as i64)) / (span_x as i64))
        .clamp(0, width.saturating_sub(1) as i64) as i32;
    let ty = (((dy - min_dy) as i64 * (height.saturating_sub(1) as i64)) / (span_y as i64))
        .clamp(0, height.saturating_sub(1) as i64) as i32;
    Some([tx, ty])
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OverlayKind {
    Ore,
    Gems,
    Bridge,
    Crate,
    Tree,
    Other,
}

fn classify_westwood_overlay_id(overlay_id: u8) -> Option<OverlayKind> {
    if overlay_id == 0xFF {
        return None;
    }
    Some(match overlay_id {
        // RA2/YR/TS Gold Ore / Riparius (`TIB01..TIB12` = 27..=38, `TIB2_*` / `CRU*` = 102..=126)
        27..=38 | 102..=126 => OverlayKind::Ore,
        // RA2/YR/TS Gems / Vinifera / Aboreus (`GEM01..GEM12` = 39..=50, `ABORE*` = 127..=166)
        39..=50 | 127..=166 => OverlayKind::Gems,
        // Low/High wood, concrete & steel bridges
        24..=25 | 74..=98 | 202..=238 => OverlayKind::Bridge,
        // Goodie crates & water crates
        26 | 167 | 240..=243 => OverlayKind::Crate,
        // Trees, rocks & props
        60..=73 | 168..=201 => OverlayKind::Tree,
        _ => OverlayKind::Other,
    })
}

/// Decodes `[IsoMapPack5]`, `[OverlayPack]`, `[OverlayDataPack]`, and `[Triggers]`/`[Events]`/`[Actions]`
/// from a Westwood `.map` / `.mpr` INI into genuine heightmap, PBR splatmap, terrain classification tiles,
/// ore/gem resource tiles, bridges, and `TriggerZoneDef`s.
pub fn decode_westwood_map_packs(
    ini: &WestwoodIni,
    width: u32,
    height: u32,
    waypoints: &BTreeMap<i32, [i32; 2]>,
    known_units: &[UnitDef],
    factions: &FactionCatalog,
) -> Result<Option<DecodedIsoMapPack>> {
    let iso_b64 = decode_ini_section_base64(ini, "IsoMapPack5")?;
    let ovl_b64 = decode_ini_section_base64(ini, "OverlayPack")?;
    let has_triggers = ini
        .sections
        .keys()
        .any(|k| k.eq_ignore_ascii_case("Triggers"));

    if iso_b64.is_none() && ovl_b64.is_none() && !has_triggers {
        return Ok(None);
    }

    let w = width.clamp(16, 256);
    let h = height.clamp(16, 256);
    let t_count = (w * h) as usize;

    let raw_size_parts: Vec<i32> = ini
        .get("Map", "Size")
        .unwrap_or("0,0,64,64")
        .split(',')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect();
    let map_raw_w = raw_size_parts.get(2).copied().unwrap_or(w as i32).max(16);

    let tile_records = if let Some(ref packed) = iso_b64 {
        let unpacked = decompress_westwood_pack_chunks(packed)?;
        parse_isomappack5_records(&unpacked)
    } else {
        Vec::new()
    };

    let direct_grid_mode = !tile_records.is_empty()
        && tile_records
            .iter()
            .all(|r| r.rx >= 0 && r.ry >= 0 && (r.rx as u32) < w && (r.ry as u32) < h);

    let iso_bounds = if !direct_grid_mode && !tile_records.is_empty() {
        let mut min_dx = i32::MAX;
        let mut max_dx = i32::MIN;
        let mut min_dy = i32::MAX;
        let mut max_dy = i32::MIN;
        for r in &tile_records {
            let dx = (r.rx as i32) - (r.ry as i32) + map_raw_w - 1;
            let dy = (r.rx as i32) + (r.ry as i32) - map_raw_w - 1;
            min_dx = min_dx.min(dx);
            max_dx = max_dx.max(dx);
            min_dy = min_dy.min(dy);
            max_dy = max_dy.max(dy);
        }
        (min_dx, max_dx, min_dy, max_dy)
    } else {
        (0, (w as i32) * 2, 0, (h as i32) * 2)
    };

    // Determine modal base ground level `base_z`
    let mut z_hist = [0u32; 256];
    for r in &tile_records {
        z_hist[r.z_height as usize] += 1;
    }
    let base_z = z_hist
        .iter()
        .enumerate()
        .max_by_key(|&(_, &cnt)| cnt)
        .map(|(z, _)| z as u8)
        .unwrap_or(0);

    let mut tile_z = vec![base_z; t_count];
    let mut tile_set_num = vec![0u32; t_count];
    let mut water_set = BTreeSet::new();
    let mut elev_set = BTreeSet::new();
    let mut ramp_set = BTreeSet::new();

    for rec in &tile_records {
        let Some([tx, ty]) = map_iso_coord_to_grid(
            rec.rx,
            rec.ry,
            direct_grid_mode,
            iso_bounds,
            map_raw_w,
            w,
            h,
        ) else {
            continue;
        };
        let idx = (ty as u32 * w + tx as u32) as usize;
        tile_z[idx] = rec.z_height;
        tile_set_num[idx] = rec.tile_num;

        let is_water_tileset = (59..=82).contains(&rec.tile_num) && rec.z_height <= base_z;
        let is_ramp_tileset = (150..=189).contains(&rec.tile_num);

        if (base_z > 0 && rec.z_height < base_z) || is_water_tileset {
            water_set.insert([tx, ty]);
        } else if rec.z_height >= base_z.saturating_add(2) {
            elev_set.insert([tx, ty]);
        } else if rec.z_height == base_z.saturating_add(1) || is_ramp_tileset {
            ramp_set.insert([tx, ty]);
        }
    }

    // Decode `[OverlayPack]` + optional `[OverlayDataPack]`
    let mut ore_set = BTreeMap::<[i32; 2], u8>::new();
    let mut gem_set = BTreeMap::<[i32; 2], u8>::new();
    let mut bridge_set = BTreeSet::new();
    let mut crate_set = BTreeSet::new();
    let mut tree_set = BTreeSet::new();

    if let Some(ref ovl_packed) = ovl_b64 {
        let ovl_bytes = decompress_westwood_pack_chunks(ovl_packed)?;
        let ovl_data = match decode_ini_section_base64(ini, "OverlayDataPack")? {
            Some(dp) => decompress_westwood_pack_chunks(&dp).unwrap_or_default(),
            None => Vec::new(),
        };

        if ovl_bytes.len() >= 262_144 {
            // Classic 512x512 Westwood isometric overlay matrix (`ry * 512 + rx`)
            for ry in 0..512i16 {
                for rx in 0..512i16 {
                    let cell_i = (ry as usize) * 512 + (rx as usize);
                    let ov_id = ovl_bytes[cell_i];
                    let Some(kind) = classify_westwood_overlay_id(ov_id) else {
                        continue;
                    };
                    let is_direct = direct_grid_mode && (rx as u32) < w && (ry as u32) < h;
                    let Some(coord) =
                        map_iso_coord_to_grid(rx, ry, is_direct, iso_bounds, map_raw_w, w, h)
                    else {
                        continue;
                    };
                    let density = ovl_data.get(cell_i).copied().unwrap_or(6);
                    match kind {
                        OverlayKind::Ore => {
                            ore_set.insert(coord, density);
                        }
                        OverlayKind::Gems => {
                            gem_set.insert(coord, density);
                        }
                        OverlayKind::Bridge => {
                            bridge_set.insert(coord);
                        }
                        OverlayKind::Crate => {
                            crate_set.insert(coord);
                        }
                        OverlayKind::Tree => {
                            tree_set.insert(coord);
                        }
                        OverlayKind::Other => {}
                    }
                }
            }
        } else {
            // Rectangular `w * h` overlay grid
            for (cell_i, &ov_id) in ovl_bytes.iter().take(t_count).enumerate() {
                let Some(kind) = classify_westwood_overlay_id(ov_id) else {
                    continue;
                };
                let tx = (cell_i as u32 % w) as i32;
                let ty = (cell_i as u32 / w) as i32;
                let coord = [tx, ty];
                let density = ovl_data.get(cell_i).copied().unwrap_or(6);
                match kind {
                    OverlayKind::Ore => {
                        ore_set.insert(coord, density);
                    }
                    OverlayKind::Gems => {
                        gem_set.insert(coord, density);
                    }
                    OverlayKind::Bridge => {
                        bridge_set.insert(coord);
                    }
                    OverlayKind::Crate => {
                        crate_set.insert(coord);
                    }
                    OverlayKind::Tree => {
                        tree_set.insert(coord);
                    }
                    OverlayKind::Other => {}
                }
            }
        }
    }

    // Build `(w + 1) x (h + 1)` continuous vertex heightmap & 4-channel RGBA PBR splatmap
    let vw = w + 1;
    let vh = h + 1;
    let v_count = (vw * vh) as usize;
    let mut tile_world_h = vec![0.0f32; t_count];
    for ty in 0..h {
        for tx in 0..w {
            let idx = (ty * w + tx) as usize;
            let coord = [tx as i32, ty as i32];
            if water_set.contains(&coord) && !bridge_set.contains(&coord) {
                tile_world_h[idx] = -0.65;
            } else {
                let dz = (tile_z[idx] as i32) - (base_z as i32);
                tile_world_h[idx] = (dz as f32) * 0.75;
            }
        }
    }

    let mut heights = vec![0.0f32; v_count];
    for vz in 0..vh {
        for vx in 0..vw {
            let mut sum = 0.0f32;
            let mut cnt = 0u32;
            for &(ox, oz) in &[(-1i32, -1i32), (0, -1), (-1, 0), (0, 0)] {
                let tx = vx as i32 + ox;
                let tz = vz as i32 + oz;
                if tx >= 0 && tx < w as i32 && tz >= 0 && tz < h as i32 {
                    sum += tile_world_h[(tz as u32 * w + tx as u32) as usize];
                    cnt += 1;
                }
            }
            heights[(vz * vw + vx) as usize] = if cnt > 0 { sum / cnt as f32 } else { 0.0 };
        }
    }
    let heightmap = HeightmapDef {
        width: vw,
        height: vh,
        heights,
    };

    let mut weights_rgba = Vec::with_capacity(v_count);
    for vz in 0..vh {
        for vx in 0..vw {
            let tx = vx.min(w - 1);
            let tz = vz.min(h - 1);
            let tidx = (tz * w + tx) as usize;
            let coord = [tx as i32, tz as i32];

            let h_l = heightmap.sample_vertex(vx.saturating_sub(1), vz);
            let h_r = heightmap.sample_vertex((vx + 1).min(vw - 1), vz);
            let h_d = heightmap.sample_vertex(vx, vz.saturating_sub(1));
            let h_u = heightmap.sample_vertex(vx, (vz + 1).min(vh - 1));
            let h_c = heightmap.sample_vertex(vx, vz);
            let slope = (h_r - h_l).abs() + (h_u - h_d).abs();

            let mut w_grass = 0.80f32;
            let mut w_dirt = 0.10f32;
            let mut w_concrete = 0.0f32;
            let mut w_rock = 0.0f32;

            if bridge_set.contains(&coord) || tile_set_num[tidx] >= 200 {
                w_concrete += 1.50;
                w_grass *= 0.20;
            }
            if ore_set.contains_key(&coord)
                || gem_set.contains_key(&coord)
                || ramp_set.contains(&coord)
                || (h_c < -0.05 && !bridge_set.contains(&coord))
            {
                w_dirt += 1.45;
                w_grass *= 0.20;
            }
            if slope > 0.55 {
                w_rock += (slope * 1.6).max(1.25);
                w_grass *= 0.15;
            }

            let sum = (w_grass + w_dirt + w_concrete + w_rock).max(0.0001);
            let r = ((w_grass / sum) * 255.0).round().clamp(0.0, 255.0) as u8;
            let g = ((w_dirt / sum) * 255.0)
                .round()
                .clamp(0.0, (255 - r) as f32) as u8;
            let b = ((w_concrete / sum) * 255.0)
                .round()
                .clamp(0.0, (255 - r - g) as f32) as u8;
            let a = 255u8.saturating_sub(r).saturating_sub(g).saturating_sub(b);
            weights_rgba.push([r, g, b, a]);
        }
    }

    let splatmap = TerrainSplatDef {
        layers: vec![
            "grass".to_string(),
            "dirt_ore".to_string(),
            "concrete".to_string(),
            "cliff_rock".to_string(),
        ],
        weights_rgba,
    };

    let trigger_zones = parse_westwood_map_triggers(ini, w, h, waypoints, known_units, factions);

    Ok(Some(DecodedIsoMapPack {
        width: w,
        height: h,
        tile_records,
        heightmap,
        splatmap,
        water_tiles: water_set.into_iter().collect(),
        elevation_tiles: elev_set.into_iter().collect(),
        ramp_tiles: ramp_set.into_iter().collect(),
        ore_tiles: ore_set.keys().copied().collect(),
        gem_tiles: gem_set.keys().copied().collect(),
        bridge_tiles: bridge_set.into_iter().collect(),
        crate_tiles: crate_set.into_iter().collect(),
        tree_tiles: tree_set.into_iter().collect(),
        trigger_zones,
    }))
}

/// Applies decoded `[IsoMapPack5]`, `[OverlayPack]`, and `[Triggers]` data onto a `MapDef`,
/// automatically protecting player spawn & preplaced entity footprints from water/obstacle collisions.
pub fn apply_isomappack_to_map_def(map_def: &mut MapDef, decoded: &DecodedIsoMapPack) {
    let w = map_def.width;
    let h = map_def.height;
    if decoded.heightmap.width == w + 1
        && decoded.heightmap.height == h + 1
        && decoded.heightmap.heights.len() == ((w + 1) * (h + 1)) as usize
    {
        map_def.heightmap = Some(decoded.heightmap.clone());
    }
    if decoded.splatmap.weights_rgba.len() == ((w + 1) * (h + 1)) as usize {
        map_def.splatmap = Some(decoded.splatmap.clone());
    }

    let is_near_spawn_or_entity = |pt: [i32; 2]| -> bool {
        let hits_spawn = map_def.spawns.iter().any(|sp| {
            let near_base =
                (pt[0] - sp.base_tile[0]).abs() <= 4 && (pt[1] - sp.base_tile[1]).abs() <= 4;
            let near_bld = sp.initial_buildings.iter().any(|ib| {
                (pt[0] - ib.tile_pos[0]).abs() <= 3 && (pt[1] - ib.tile_pos[1]).abs() <= 3
            });
            let near_unit = sp.initial_units.iter().any(|iu| {
                (pt[0] - iu.tile_pos[0]).abs() <= 1 && (pt[1] - iu.tile_pos[1]).abs() <= 1
            });
            near_base || near_bld || near_unit
        });
        let hits_preplaced = map_def
            .preplaced_entities
            .iter()
            .any(|pe| (pt[0] - pe.tile_pos[0]).abs() <= 2 && (pt[1] - pe.tile_pos[1]).abs() <= 2);
        hits_spawn || hits_preplaced
    };

    let safe_water: Vec<[i32; 2]> = decoded
        .water_tiles
        .iter()
        .copied()
        .filter(|&pt| !is_near_spawn_or_entity(pt))
        .collect();
    if !safe_water.is_empty() {
        map_def.water_rects = merge_tiles_into_rects(&safe_water, w, h);
    }

    if !decoded.bridge_tiles.is_empty() {
        map_def.bridge_rects = merge_tiles_into_rects(&decoded.bridge_tiles, w, h);
    }
    if !decoded.elevation_tiles.is_empty() {
        map_def.elevation_rects = merge_tiles_into_rects(&decoded.elevation_tiles, w, h);
    }
    if !decoded.ramp_tiles.is_empty() {
        map_def.ramp_rects = merge_tiles_into_rects(&decoded.ramp_tiles, w, h);
    }

    if !decoded.ore_tiles.is_empty() {
        let ore_with_d: Vec<([i32; 2], u8)> = decoded.ore_tiles.iter().map(|&pt| (pt, 6)).collect();
        map_def.ore_patches = cluster_resource_tiles_into_patches(&ore_with_d, 600, w, h);
    }
    if !decoded.gem_tiles.is_empty() {
        let gem_with_d: Vec<([i32; 2], u8)> = decoded.gem_tiles.iter().map(|&pt| (pt, 8)).collect();
        map_def.gem_patches = cluster_resource_tiles_into_patches(&gem_with_d, 1000, w, h);
    }

    for &tree_pt in decoded.tree_tiles.iter().take(64) {
        if !is_near_spawn_or_entity(tree_pt) && !safe_water.contains(&tree_pt) {
            map_def.props.push(MapPropDef {
                kind: PropKind::Tree,
                tile_pos: tree_pt,
            });
        }
    }

    if !decoded.trigger_zones.is_empty() {
        // Ensure any SpawnReinforcements action tile does not land on unbridged water or obstacles
        let mut sanitized_zones = decoded.trigger_zones.clone();
        for z in &mut sanitized_zones {
            for act in &mut z.actions {
                if let TriggerAction::SpawnReinforcements { tile, .. } = act {
                    let on_water =
                        safe_water.contains(tile) && !decoded.bridge_tiles.contains(tile);
                    let on_obstacle = map_def.obstacles.iter().any(|obs| {
                        tile[0] >= obs.min[0]
                            && tile[0] <= obs.max[0]
                            && tile[1] >= obs.min[1]
                            && tile[1] <= obs.max[1]
                    });
                    if on_water || on_obstacle {
                        if let Some(sp) = map_def.spawns.first() {
                            *tile = sp.base_tile;
                        }
                    }
                }
            }
        }
        map_def.trigger_zones = sanitized_zones;
    }
}
