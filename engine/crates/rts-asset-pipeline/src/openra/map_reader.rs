use super::miniyaml::*;
use crate::westwood::build_default_converted_map;
use crate::westwood::normalize_id;
use anyhow::{bail, Result};
use rts_sdk::schema::{
    BuildingDef, InitialBuildingSpawn, InitialUnitSpawn, MapDef, ObstacleRectDef, OrePatchDef,
    PlayerSpawnDef, UnitDef,
};

fn merge_bool_grid_to_rects(
    grid: &[bool],
    gw: usize,
    gh: usize,
    min_area: usize,
) -> Vec<ObstacleRectDef> {
    let mut visited = vec![false; gw * gh];
    let mut rects = Vec::new();
    for y0 in 0..gh {
        for x0 in 0..gw {
            let idx = y0 * gw + x0;
            if !grid[idx] || visited[idx] {
                continue;
            }
            let mut x1 = x0;
            while x1 + 1 < gw && grid[y0 * gw + (x1 + 1)] && !visited[y0 * gw + (x1 + 1)] {
                x1 += 1;
            }
            let mut y1 = y0;
            'grow_y: while y1 + 1 < gh {
                for x in x0..=x1 {
                    let c = (y1 + 1) * gw + x;
                    if !grid[c] || visited[c] {
                        break 'grow_y;
                    }
                }
                y1 += 1;
            }
            for y in y0..=y1 {
                for x in x0..=x1 {
                    visited[y * gw + x] = true;
                }
            }
            let area = (x1 - x0 + 1) * (y1 - y0 + 1);
            if area >= min_area {
                rects.push(ObstacleRectDef {
                    min: [x0 as i32, y0 as i32],
                    max: [x1 as i32, y1 as i32],
                });
            }
        }
    }
    rects
}

pub fn convert_openra_map_yaml(
    map_yaml: &str,
    fallback_buildings: &[BuildingDef],
    fallback_units: &[UnitDef],
) -> MapDef {
    let nodes = parse_miniyaml(map_yaml);
    let mut map = build_default_converted_map(fallback_buildings, fallback_units);

    for node in &nodes {
        match node.key.as_str() {
            "Title" if !node.value.is_empty() => {
                map.name = node.value.clone();
                map.id = normalize_id(&node.value);
            }
            "MapSize" => {
                if let Some((w, h)) = node.value.split_once(',') {
                    if let (Ok(pw), Ok(ph)) = (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
                        map.width = pw.clamp(20, 128);
                        map.height = ph.clamp(20, 128);
                    }
                }
            }
            "Bounds" => {
                let parts: Vec<&str> = node.value.split(',').collect();
                if parts.len() == 4 {
                    if let (Ok(pw), Ok(ph)) = (
                        parts[2].trim().parse::<u32>(),
                        parts[3].trim().parse::<u32>(),
                    ) {
                        map.width = pw.clamp(20, 128);
                        map.height = ph.clamp(20, 128);
                    }
                }
            }
            "Actors" => {
                let mut spawn_coords: Vec<[i32; 2]> = Vec::new();
                let mut ore_patches: Vec<OrePatchDef> = Vec::new();
                for actor in &node.children {
                    let actor_type = actor.value.trim().to_ascii_lowercase();
                    let Some(loc_str) = actor.child_value("Location") else {
                        continue;
                    };
                    let Some((lx, ly)) = loc_str.split_once(',') else {
                        continue;
                    };
                    let (Ok(x), Ok(y)) = (lx.trim().parse::<i32>(), ly.trim().parse::<i32>())
                    else {
                        continue;
                    };
                    if actor_type == "mpspawn" {
                        spawn_coords.push([x, y]);
                    } else if actor_type.contains("mine") || actor_type.contains("tibtre") {
                        ore_patches.push(OrePatchDef {
                            position: [x, y],
                            radius: 3,
                            amount_per_tile: 600,
                        });
                    } else if actor_type.contains("cabhut") && map.water_rects.is_empty() {
                        let cx = (map.width as i32) / 2;
                        let cy = (map.height as i32) / 2;
                        map.water_rects.push(ObstacleRectDef {
                            min: [(cx - 8).max(2), (cy + 4).min(map.height as i32 - 6)],
                            max: [
                                (cx + 8).min(map.width as i32 - 3),
                                (cy + 8).min(map.height as i32 - 3),
                            ],
                        });
                    }
                }
                if !ore_patches.is_empty() {
                    map.ore_patches = ore_patches;
                }
                if !spawn_coords.is_empty() {
                    let templates = map.spawns.clone();
                    let mut new_spawns = Vec::new();
                    for (idx, pos) in spawn_coords.into_iter().take(8).enumerate() {
                        let mut sp = templates
                            .get(idx % templates.len().max(1))
                            .cloned()
                            .unwrap_or(PlayerSpawnDef {
                                player_id: (idx + 1) as u8,
                                faction: "allies".to_string(),
                                sub_faction: None,
                                base_tile: pos,
                                team_id: 0,
                                initial_buildings: Vec::new(),
                                initial_units: Vec::new(),
                            });
                        sp.player_id = (idx + 1) as u8;
                        sp.base_tile = pos;
                        new_spawns.push(sp);
                    }
                    if new_spawns.len() >= 2 {
                        map.spawns = new_spawns;
                    } else if let Some(first_sp) = map.spawns.first_mut() {
                        first_sp.base_tile = new_spawns[0].base_tile;
                    }
                }
            }
            _ => {}
        }
    }

    // Ensure spawns, ore patches, and initial entities are within map bounds without clobbering valid spawns
    let max_x = (map.width as i32 - 4).max(4);
    let max_y = (map.height as i32 - 4).max(4);
    for ore in &mut map.ore_patches {
        ore.position[0] = ore.position[0].clamp(4, max_x);
        ore.position[1] = ore.position[1].clamp(4, max_y);
    }

    let mut placed_bases: Vec<[i32; 2]> = Vec::new();
    let fallback_corners = [
        [6, 6],
        [(max_x - 6).max(6), (max_y - 6).max(6)],
        [(max_x - 6).max(6), 6],
        [6, (max_y - 6).max(6)],
        [(map.width as i32) / 2, 6],
        [(map.width as i32) / 2, (max_y - 6).max(6)],
        [6, (map.height as i32) / 2],
        [(max_x - 6).max(6), (map.height as i32) / 2],
    ];
    let unit_offsets = [[4, 1], [4, 3], [5, 2], [3, 4]];

    for (idx, sp) in map.spawns.iter_mut().enumerate() {
        sp.base_tile[0] = sp.base_tile[0].clamp(4, (max_x - 3).max(4));
        sp.base_tile[1] = sp.base_tile[1].clamp(4, (max_y - 3).max(4));

        // Only nudge if isometric clamping caused two distinct spawns to collide on the same base area
        let collides = placed_bases.iter().any(|prev| {
            (prev[0] - sp.base_tile[0]).abs() <= 4 && (prev[1] - sp.base_tile[1]).abs() <= 4
        });
        if collides {
            if let Some(&alt) = fallback_corners.iter().find(|cand| {
                placed_bases
                    .iter()
                    .all(|p| (p[0] - cand[0]).abs() > 4 || (p[1] - cand[1]).abs() > 4)
            }) {
                sp.base_tile = alt;
            }
        }
        placed_bases.push(sp.base_tile);

        let sign_x = if sp.base_tile[0] <= (map.width as i32) / 2 {
            1
        } else {
            -1
        };
        let sign_y = if sp.base_tile[1] <= (map.height as i32) / 2 {
            1
        } else {
            -1
        };
        for InitialBuildingSpawn { tile_pos, .. } in &mut sp.initial_buildings {
            *tile_pos = sp.base_tile;
        }
        for (u_idx, InitialUnitSpawn { tile_pos, .. }) in sp.initial_units.iter_mut().enumerate() {
            let off = unit_offsets[u_idx % unit_offsets.len()];
            tile_pos[0] = (sp.base_tile[0] + sign_x * off[0]).clamp(2, max_x);
            tile_pos[1] = (sp.base_tile[1] + sign_y * off[1]).clamp(2, max_y);
        }
        let _ = idx;
    }

    if map.obstacles.is_empty() && map.water_rects.is_empty() {
        let cx = (map.width as i32) / 2;
        let cy = (map.height as i32) / 2;
        let hits_spawn = map
            .spawns
            .iter()
            .any(|sp| (sp.base_tile[0] - cx).abs() <= 5 && (sp.base_tile[1] - cy).abs() <= 5);
        if !hits_spawn {
            map.obstacles.push(ObstacleRectDef {
                min: [cx - 1, cy - 1],
                max: [cx + 1, cy + 1],
            });
        }
    }

    map
}

/// Parses an OpenRA binary `map.bin` file (format version 1 or 2) and enriches `map_def`
/// with binary tile dimensions, resource/ore patches, disjoint water rects, and elevation rects.
pub fn parse_openra_map_bin(bin_bytes: &[u8], map_def: &mut MapDef) -> Result<()> {
    if bin_bytes.len() < 5 {
        bail!("OpenRA map.bin is too short ({} bytes)", bin_bytes.len());
    }
    let version = bin_bytes[0];
    let width = u16::from_le_bytes(bin_bytes[1..3].try_into().unwrap()) as usize;
    let height = u16::from_le_bytes(bin_bytes[3..5].try_into().unwrap()) as usize;
    if width == 0 || height == 0 || width > 512 || height > 512 {
        bail!("Invalid OpenRA map.bin dimensions: {width}x{height}");
    }

    let num_cells = width * height;
    let (tiles_off, heights_off, res_off) = if version == 1 {
        let t_off = 5usize;
        let h_off = t_off + num_cells * 3;
        let r_off = if bin_bytes.len() >= h_off + num_cells + num_cells * 2 {
            h_off + num_cells
        } else {
            t_off + num_cells * 3
        };
        (t_off, h_off, r_off)
    } else if version == 2 && bin_bytes.len() >= 17 {
        let t_off = u32::from_le_bytes(bin_bytes[5..9].try_into().unwrap()) as usize;
        let h_off = u32::from_le_bytes(bin_bytes[9..13].try_into().unwrap()) as usize;
        let r_off = u32::from_le_bytes(bin_bytes[13..17].try_into().unwrap()) as usize;
        (t_off, h_off, r_off)
    } else {
        bail!("Unsupported OpenRA map.bin format version {version}");
    };

    map_def.width = (width as u32).clamp(20, 128);
    map_def.height = (height as u32).clamp(20, 128);
    let mw = map_def.width as usize;
    let mh = map_def.height as usize;
    let max_x = (map_def.width as i32 - 4).max(4);
    let max_y = (map_def.height as i32 - 4).max(4);

    // Parse resource cells (resource_type: u8, density: u8) into clustered OrePatchDef entries
    if res_off + num_cells * 2 <= bin_bytes.len() {
        let mut extracted_ores: Vec<OrePatchDef> = Vec::new();
        for x in 0..width {
            for y in 0..height {
                let cell_idx = x * height + y;
                let r_base = res_off + cell_idx * 2;
                let res_type = bin_bytes[r_base];
                let res_density = bin_bytes[r_base + 1];
                if res_type > 0 && res_density > 0 {
                    let sx = ((x as f32 / width as f32) * map_def.width as f32) as i32;
                    let sy = ((y as f32 / height as f32) * map_def.height as f32) as i32;
                    let cx = sx.clamp(4, max_x);
                    let cy = sy.clamp(4, max_y);
                    let already_clustered = extracted_ores.iter().any(|p| {
                        (p.position[0] - cx).abs() <= 5 && (p.position[1] - cy).abs() <= 5
                    });
                    if !already_clustered && extracted_ores.len() < 16 {
                        extracted_ores.push(OrePatchDef {
                            position: [cx, cy],
                            radius: 3,
                            amount_per_tile: (res_density as u32 * 50).clamp(400, 1200),
                        });
                    }
                }
            }
        }
        if !extracted_ores.is_empty() {
            map_def.ore_patches = extracted_ores;
        }
    }

    let is_near_ground_spawn = |tx: i32, ty: i32, map: &MapDef| -> bool {
        map.spawns.iter().any(|sp| {
            ((sp.base_tile[0] - tx).abs() <= 5 && (sp.base_tile[1] - ty).abs() <= 5)
                || sp
                    .initial_units
                    .iter()
                    .any(|u| (u.tile_pos[0] - tx).abs() <= 1 && (u.tile_pos[1] - ty).abs() <= 1)
        })
    };

    // Parse water tiles from the tile table into disjoint merged water rectangles (never single giant AABB!)
    if map_def.water_rects.is_empty() && tiles_off + num_cells * 3 <= bin_bytes.len() {
        let mut water_grid = vec![false; mw * mh];
        for x in 0..width {
            for y in 0..height {
                let cell_idx = x * height + y;
                let t_base = tiles_off + cell_idx * 3;
                let tile_type =
                    u16::from_le_bytes(bin_bytes[t_base..t_base + 2].try_into().unwrap());
                if tile_type == 1 || tile_type == 2 {
                    let sx = (((x as f32 / width as f32) * mw as f32) as usize).min(mw - 1);
                    let sy = (((y as f32 / height as f32) * mh as f32) as usize).min(mh - 1);
                    if !is_near_ground_spawn(sx as i32, sy as i32, map_def) {
                        water_grid[sy * mw + sx] = true;
                    }
                }
            }
        }
        let merged = merge_bool_grid_to_rects(&water_grid, mw, mh, 4);
        if !merged.is_empty() {
            // Remove any synthetic center obstacle that overlaps extracted water
            map_def.obstacles.retain(|obs| {
                !merged.iter().any(|w| {
                    obs.min[0] <= w.max[0]
                        && obs.max[0] >= w.min[0]
                        && obs.min[1] <= w.max[1]
                        && obs.max[1] >= w.min[1]
                })
            });
            map_def.water_rects = merged;
        }
    }

    // Parse height table into elevation plateaus
    if map_def.elevation_rects.is_empty()
        && heights_off < res_off
        && heights_off + num_cells <= bin_bytes.len()
    {
        let mut high_grid = vec![false; mw * mh];
        for x in 0..width {
            for y in 0..height {
                let cell_idx = x * height + y;
                let h = bin_bytes[heights_off + cell_idx];
                if h >= 2 {
                    let sx = (((x as f32 / width as f32) * mw as f32) as usize).min(mw - 1);
                    let sy = (((y as f32 / height as f32) * mh as f32) as usize).min(mh - 1);
                    high_grid[sy * mw + sx] = true;
                }
            }
        }
        map_def
            .elevation_rects
            .extend(merge_bool_grid_to_rects(&high_grid, mw, mh, 4));
    }

    Ok(())
}

/// Builds a synthetic valid OpenRA format-1 `map.bin` binary buffer for testing binary map parsing.
pub fn build_sample_openra_map_bin(width: u16, height: u16) -> Vec<u8> {
    let num_cells = (width as usize) * (height as usize);
    let mut bin = Vec::with_capacity(5 + num_cells * 6);
    bin.push(1u8); // format version 1
    bin.extend_from_slice(&width.to_le_bytes());
    bin.extend_from_slice(&height.to_le_bytes());

    // Tile table: num_cells * 3 bytes (tile_type: u16, index: u8)
    for x in 0..width {
        for y in 0..height {
            let is_water = x >= width / 2 - 2
                && x <= width / 2 + 2
                && y >= height / 2 - 2
                && y <= height / 2 + 2;
            let tile_type: u16 = if is_water { 1 } else { 0 };
            bin.extend_from_slice(&tile_type.to_le_bytes());
            bin.push(0u8);
        }
    }

    // Height table: num_cells * 1 byte
    bin.resize(bin.len() + num_cells, 0u8);

    // Resource table: num_cells * 2 bytes (resource_type: u8, density: u8)
    for x in 0..width {
        for y in 0..height {
            if (x == 10 && y == 10) || (x == width - 10 && y == height - 10) {
                bin.push(1u8); // Ore/Gems resource_type
                bin.push(12u8); // density
            } else {
                bin.push(0u8);
                bin.push(0u8);
            }
        }
    }

    bin
}
