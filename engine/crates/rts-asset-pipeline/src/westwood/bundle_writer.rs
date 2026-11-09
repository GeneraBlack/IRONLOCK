use super::*;
use crate::mesh_builder::generate_mod_models;
use anyhow::Result;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    BuildingDef, BuildingRole, MapDef,
    ObstacleRectDef, OrePatchDef, PlayerSpawnDef, UnitDef, UnitRole,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn build_default_converted_map(buildings: &[BuildingDef], units: &[UnitDef]) -> MapDef {
    let faction_catalog = build_dynamic_faction_catalog(buildings, units);
    let p1_faction = faction_catalog
        .factions
        .first()
        .map(|f| f.id.as_str())
        .unwrap_or("allies");
    let p2_faction = faction_catalog
        .factions
        .get(1)
        .or_else(|| faction_catalog.factions.first())
        .map(|f| f.id.as_str())
        .unwrap_or("soviets");

    let allied_conyard = buildings
        .iter()
        .find(|b| b.is_available_to_faction(p1_faction) && b.role == BuildingRole::ConstructionYard)
        .or_else(|| {
            buildings
                .iter()
                .find(|b| b.role == BuildingRole::ConstructionYard)
        })
        .or_else(|| buildings.first())
        .map(|b| b.id.clone());
    let soviet_conyard = buildings
        .iter()
        .find(|b| b.is_available_to_faction(p2_faction) && b.role == BuildingRole::ConstructionYard)
        .or_else(|| {
            buildings
                .iter()
                .find(|b| b.role == BuildingRole::ConstructionYard)
        })
        .or_else(|| buildings.first())
        .map(|b| b.id.clone());

    let allied_inf = units
        .iter()
        .find(|u| u.is_available_to_faction(p1_faction) && u.role == UnitRole::Infantry)
        .or_else(|| units.iter().find(|u| u.is_available_to_faction(p1_faction)))
        .or_else(|| units.first())
        .map(|u| u.id.clone());
    let allied_tank = units
        .iter()
        .find(|u| u.is_available_to_faction(p1_faction) && u.role == UnitRole::Vehicle)
        .map(|u| u.id.clone());
    let allied_harv = units
        .iter()
        .find(|u| u.is_available_to_faction(p1_faction) && u.role == UnitRole::Harvester)
        .map(|u| u.id.clone());

    let soviet_inf = units
        .iter()
        .find(|u| u.is_available_to_faction(p2_faction) && u.role == UnitRole::Infantry)
        .or_else(|| units.iter().find(|u| u.is_available_to_faction(p2_faction)))
        .or_else(|| units.first())
        .map(|u| u.id.clone());
    let soviet_tank = units
        .iter()
        .find(|u| u.is_available_to_faction(p2_faction) && u.role == UnitRole::Vehicle)
        .map(|u| u.id.clone());
    let soviet_harv = units
        .iter()
        .find(|u| u.is_available_to_faction(p2_faction) && u.role == UnitRole::Harvester)
        .map(|u| u.id.clone());

    let mut p1_buildings = Vec::new();
    if let Some(b_id) = allied_conyard {
        p1_buildings.push(rts_sdk::schema::InitialBuildingSpawn {
            building_id: b_id,
            tile_pos: [6, 6],
        });
    }
    let mut p1_units = Vec::new();
    if let Some(u_id) = allied_inf {
        p1_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: u_id.clone(),
            tile_pos: [10, 7],
        });
        p1_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: u_id,
            tile_pos: [10, 9],
        });
    }
    if let Some(t_id) = allied_tank {
        p1_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: t_id,
            tile_pos: [11, 8],
        });
    }
    if let Some(h_id) = allied_harv {
        p1_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: h_id,
            tile_pos: [9, 10],
        });
    }

    let mut p2_buildings = Vec::new();
    if let Some(b_id) = soviet_conyard {
        p2_buildings.push(rts_sdk::schema::InitialBuildingSpawn {
            building_id: b_id,
            tile_pos: [32, 32],
        });
    }
    let mut p2_units = Vec::new();
    if let Some(u_id) = soviet_inf {
        p2_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: u_id.clone(),
            tile_pos: [29, 30],
        });
        p2_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: u_id,
            tile_pos: [29, 32],
        });
    }
    if let Some(t_id) = soviet_tank {
        p2_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: t_id,
            tile_pos: [28, 31],
        });
    }
    if let Some(h_id) = soviet_harv {
        p2_units.push(rts_sdk::schema::InitialUnitSpawn {
            unit_id: h_id,
            tile_pos: [30, 29],
        });
    }

    MapDef {
        id: "converted_skirmish".to_string(),
        name: "Converted Skirmish Arena".to_string(),
        width: 40,
        height: 40,
        tile_size: 2.0,
        ore_patches: vec![
            OrePatchDef {
                position: [12, 10],
                radius: 3,
                amount_per_tile: 600,
            },
            OrePatchDef {
                position: [26, 28],
                radius: 3,
                amount_per_tile: 600,
            },
        ],
        obstacles: vec![ObstacleRectDef {
            min: [18, 18],
            max: [21, 21],
        }],
        water_rects: Vec::new(),
        elevation_rects: Vec::new(),
        ramp_rects: Vec::new(),
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                faction: p1_faction.to_string(),
                sub_faction: None,
                base_tile: [6, 6],
                team_id: 0,
                initial_buildings: p1_buildings,
                initial_units: p1_units,
            },
            PlayerSpawnDef {
                player_id: 2,
                faction: p2_faction.to_string(),
                sub_faction: None,
                base_tile: [32, 32],
                team_id: 0,
                initial_buildings: p2_buildings,
                initial_units: p2_units,
            },
        ],
        ..Default::default()
    }
}

pub const AUTO_TRAITS_RHAI_SCRIPT: &str = r#"// Auto-generated Rhai trait hooks for converted Westwood / OpenRA mods
fn on_self_heal_tick(ctx) {
    if ctx.health > 0.0 && ctx.health < ctx.max_health {
        let next = ctx.health + 0.35;
        if next > ctx.max_health {
            ctx.health = ctx.max_health;
        } else {
            ctx.health = next;
        }
    }
    ctx
}

fn on_custom_deploy(ctx) {
    ctx.deployed = !ctx.deployed;
    ctx
}
"#;

pub fn write_converted_bundle_to_dir(
    bundle: &ConvertedModBundle,
    out_dir: impl AsRef<Path>,
    vxl_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let root = out_dir.as_ref();
    fs::create_dir_all(root.join("factions"))?;
    fs::create_dir_all(root.join("weapons"))?;
    fs::create_dir_all(root.join("buildings"))?;
    fs::create_dir_all(root.join("units"))?;
    fs::create_dir_all(root.join("maps"))?;
    fs::create_dir_all(root.join("assets/models"))?;

    if !bundle.manifest.scripts.is_empty() {
        fs::create_dir_all(root.join("scripts"))?;
        for script_rel in &bundle.manifest.scripts {
            let script_path = root.join(script_rel);
            if let Some(parent) = script_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&script_path, AUTO_TRAITS_RHAI_SCRIPT)?;
        }
    }

    fs::write(
        root.join("mod.toml"),
        toml::to_string_pretty(&bundle.manifest)?,
    )?;
    fs::write(
        root.join(&bundle.manifest.catalogs.factions),
        toml::to_string_pretty(&bundle.factions)?,
    )?;
    fs::write(
        root.join(&bundle.manifest.catalogs.weapons),
        toml::to_string_pretty(&bundle.weapons)?,
    )?;
    fs::write(
        root.join(&bundle.manifest.catalogs.buildings),
        toml::to_string_pretty(&bundle.buildings)?,
    )?;
    fs::write(
        root.join(&bundle.manifest.catalogs.units),
        toml::to_string_pretty(&bundle.units)?,
    )?;
    fs::write(
        root.join(&bundle.manifest.entry_map),
        toml::to_string_pretty(&bundle.map)?,
    )?;

    // Generate procedural .glb models for all buildings/units first, then overwrite with any .vxl/.hva or .shp meshes
    let loaded = LoadedMod::load_from_dir(root)?;
    let mut written_models = generate_mod_models(&loaded, true)?;

    if let Some(vdir) = vxl_dir {
        if vdir.is_dir() {
            let mut vxl_files: BTreeMap<String, PathBuf> = BTreeMap::new();
            let mut hva_files: BTreeMap<String, PathBuf> = BTreeMap::new();
            let mut shp_files: BTreeMap<String, PathBuf> = BTreeMap::new();
            let mut pal_bytes: Option<Vec<u8>> = None;

            for entry in fs::read_dir(vdir)? {
                let entry = entry?;
                let path = entry.path();
                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("asset");
                let id = normalize_id(stem);
                if ext.eq_ignore_ascii_case("vxl") {
                    vxl_files.insert(id, path);
                } else if ext.eq_ignore_ascii_case("hva") {
                    hva_files.insert(id, path);
                } else if ext.eq_ignore_ascii_case("shp") {
                    shp_files.insert(id, path);
                } else if ext.eq_ignore_ascii_case("pal") {
                    let is_preferred_pal = matches!(
                        id.as_str(),
                        "unittem" | "temperat" | "isotem" | "isotemp" | "unitsno" | "uniturb"
                    );
                    if pal_bytes.is_none() || is_preferred_pal {
                        if let Ok(bytes) = fs::read(&path) {
                            if bytes.len() >= 768 {
                                pal_bytes = Some(bytes);
                            }
                        }
                    }
                }
            }

            let mut team_colors: BTreeMap<String, [f32; 3]> = BTreeMap::new();
            for b in &bundle.buildings.buildings {
                team_colors.insert(b.id.clone(), b.visual.primary_color);
            }
            for u in &bundle.units.units {
                team_colors.insert(u.id.clone(), u.visual.primary_color);
            }

            let mut consumed_subparts = std::collections::BTreeSet::new();
            for id in vxl_files.keys() {
                if (id.ends_with("tur") || id.ends_with("barl")) && id.len() > 3 {
                    let base = id
                        .trim_end_matches("tur")
                        .trim_end_matches("barl")
                        .to_string();
                    if vxl_files.contains_key(&base) {
                        consumed_subparts.insert(id.clone());
                    }
                }
            }

            for (id, path) in &vxl_files {
                if consumed_subparts.contains(id) {
                    continue;
                }
                let hull_bytes = fs::read(path)?;
                let mut parts: Vec<(Vec<u8>, [f32; 3])> = Vec::new();
                let hull_offset = hva_files
                    .get(id)
                    .and_then(|p| fs::read(p).ok())
                    .and_then(|b| parse_hva_offsets(&b).into_iter().next())
                    .unwrap_or([0.0, 0.0, 0.0]);
                parts.push((hull_bytes, hull_offset));

                let tur_id = format!("{id}tur");
                if let Some(tur_path) = vxl_files.get(&tur_id) {
                    if let Ok(tur_bytes) = fs::read(tur_path) {
                        let tur_off = hva_files
                            .get(&tur_id)
                            .and_then(|p| fs::read(p).ok())
                            .and_then(|b| parse_hva_offsets(&b).into_iter().next())
                            .unwrap_or([0.0, 0.35, 0.0]);
                        parts.push((tur_bytes, tur_off));
                    }
                }

                let barl_id = format!("{id}barl");
                if let Some(barl_path) = vxl_files.get(&barl_id) {
                    if let Ok(barl_bytes) = fs::read(barl_path) {
                        let barl_off = hva_files
                            .get(&barl_id)
                            .and_then(|p| fs::read(p).ok())
                            .and_then(|b| parse_hva_offsets(&b).into_iter().next())
                            .unwrap_or([0.0, 0.42, 0.45]);
                        parts.push((barl_bytes, barl_off));
                    }
                }

                let team_col = team_colors.get(id).copied();
                let refs: Vec<(&[u8], [f32; 3])> =
                    parts.iter().map(|(b, off)| (b.as_slice(), *off)).collect();
                let glb_bytes = convert_multipart_vxl_to_glb_with_team_color(
                    &refs,
                    pal_bytes.as_deref(),
                    team_col,
                    id,
                )?;

                let out_glb = root.join(format!("assets/models/{id}.glb"));
                fs::write(&out_glb, glb_bytes)?;
                if !written_models.contains(&out_glb) {
                    written_models.push(out_glb);
                }
            }

            // Convert 2D .shp sprites into volumetric 3D .glb models if no .vxl model exists for that ID
            for (id, shp_path) in &shp_files {
                if vxl_files.contains_key(id) {
                    continue;
                }
                if id.contains("icon")
                    || id.contains("pip")
                    || matches!(
                        id.as_str(),
                        "mpspawn"
                            | "camera"
                            | "waypoint"
                            | "clock"
                            | "assaultmove"
                            | "undeploy"
                            | "isodepth"
                    )
                {
                    continue;
                }
                let out_glb = root.join(format!("assets/models/{id}.glb"));
                if let Ok(shp_bytes) = fs::read(shp_path) {
                    let is_tiny_placeholder = shp_bytes.len() >= 8
                        && u16::from_le_bytes(shp_bytes[0..2].try_into().unwrap()) == 0
                        && u16::from_le_bytes(shp_bytes[2..4].try_into().unwrap()) < 12
                        && u16::from_le_bytes(shp_bytes[4..6].try_into().unwrap()) < 12
                        && u16::from_le_bytes(shp_bytes[6..8].try_into().unwrap()) < 8;
                    if written_models.contains(&out_glb) && is_tiny_placeholder {
                        continue;
                    }
                    let team_col = team_colors.get(id).copied();
                    if let Ok(glb_bytes) = convert_shp_bytes_to_glb_with_team_color(
                        &shp_bytes,
                        pal_bytes.as_deref(),
                        team_col,
                        id,
                    ) {
                        fs::write(&out_glb, glb_bytes)?;
                        if !written_models.contains(&out_glb) {
                            written_models.push(out_glb);
                        }
                    }
                }
            }
        }
    }

    Ok(written_models)
}

