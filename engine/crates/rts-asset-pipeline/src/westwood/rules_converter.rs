use super::*;
use anyhow::{bail, Result};
use rts_sdk::manifest::{ModCatalogPaths, ModManifest};
use rts_sdk::schema::{
    BuildingCatalog, BuildingDef, BuildingRole, FactionCatalog, MapDef, OrePatchDef, UnitCatalog, UnitDef, UnitRole, VisualArchetype,
    VisualDef, WeaponCatalog, WeaponDef,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct ConvertedModBundle {
    pub manifest: ModManifest,
    pub factions: FactionCatalog,
    pub weapons: WeaponCatalog,
    pub buildings: BuildingCatalog,
    pub units: UnitCatalog,
    pub map: MapDef,
}

pub fn convert_westwood_ini_to_bundle(
    ini_text: &str,
    mod_id: &str,
    mod_name: &str,
) -> Result<ConvertedModBundle> {
    let ini = WestwoodIni::parse(ini_text);

    let mut building_sections: Vec<String> = ini.list_values("BuildingTypes");
    let mut infantry_sections: Vec<String> = ini.list_values("InfantryTypes");
    let mut vehicle_sections: Vec<String> = ini.list_values("VehicleTypes");
    let mut aircraft_sections: Vec<String> = ini.list_values("AircraftTypes");
    let infantry_set: BTreeSet<String> = infantry_sections.iter().cloned().collect();
    let aircraft_set: BTreeSet<String> = aircraft_sections.iter().cloned().collect();

    // Also auto-discover sections if BuildingTypes/InfantryTypes/VehicleTypes were omitted in partial INI snippets
    for sec in &ini.section_order {
        if building_sections.contains(sec)
            || infantry_sections.contains(sec)
            || vehicle_sections.contains(sec)
            || aircraft_sections.contains(sec)
        {
            continue;
        }
        let has_cost = ini.get(sec, "Cost").is_some();
        let has_strength = ini.get(sec, "Strength").is_some();
        if !has_cost || !has_strength {
            continue;
        }
        if ini.get(sec, "Foundation").is_some()
            || ini.get(sec, "Power").is_some()
            || ini.get(sec, "Factory").is_some()
            || parse_bool(ini.get(sec, "ConstructionYard"))
            || parse_bool(ini.get(sec, "Refinery"))
            || parse_bool(ini.get(sec, "IsBaseDefense"))
        {
            building_sections.push(sec.clone());
        } else if ini
            .get(sec, "Category")
            .map(|c| c.eq_ignore_ascii_case("AirPower") || c.eq_ignore_ascii_case("AirLift"))
            .unwrap_or(false)
            || parse_bool(ini.get(sec, "ConsideredAircraft"))
        {
            aircraft_sections.push(sec.clone());
        } else if parse_bool(ini.get(sec, "Crushable"))
            || ini
                .get(sec, "Category")
                .map(|c| c.eq_ignore_ascii_case("Soldier"))
                .unwrap_or(false)
        {
            infantry_sections.push(sec.clone());
        } else if ini.get(sec, "Speed").is_some() {
            vehicle_sections.push(sec.clone());
        }
    }

    let mut weapons_map: BTreeMap<String, WeaponDef> = BTreeMap::new();
    let mut raw_buildings = Vec::new();
    let mut known_building_ids = BTreeSet::new();
    let mut has_custom_script_hooks = false;

    for sec in &building_sections {
        if !ini.sections.contains_key(sec) {
            continue;
        }
        let id = normalize_id(sec);
        if !known_building_ids.insert(id.clone()) {
            continue;
        }
        let name = ini.get(sec, "Name").unwrap_or(sec).to_string();
        let (faction, sub_factions) = map_westwood_faction_and_shared(ini.get(sec, "Owner"));
        let cost = ini
            .get(sec, "Cost")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(500);
        let health = ini
            .get(sec, "Strength")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(800.0)
            .max(50.0);
        let build_time_secs = ((cost as f32) / 100.0).clamp(3.0, 25.0);
        let armor = map_westwood_armor(ini.get(sec, "Armor"), true);
        let power_delta = ini
            .get(sec, "Power")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let vision_range = ini
            .get(sec, "Sight")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|s| (s * 2.5).clamp(10.0, 35.0))
            .unwrap_or(16.0);

        let primary_weapon = ini
            .get(sec, "Primary")
            .filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case("none"))
            .map(|w| {
                let w_def = extract_weapon_def(w, &ini);
                let w_id = w_def.id.clone();
                weapons_map.entry(w_id.clone()).or_insert(w_def);
                w_id
            });

        let (role, archetype) =
            infer_building_role_and_archetype(sec, &ini, power_delta, primary_weapon.is_some());
        let footprint = ini
            .get(sec, "Foundation")
            .and_then(|f| {
                let (w, h) = f.split_once('x')?;
                Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
            })
            .unwrap_or(match role {
                BuildingRole::ConstructionYard | BuildingRole::WarFactory => [3, 3],
                BuildingRole::Defense => [1, 1],
                _ => [2, 2],
            });

        let (_, primary_color, _) = faction_display_meta(&faction);

        let raw_prereqs = parse_csv_ids(ini.get(sec, "Prerequisite"));
        let free_unit = ini.get(sec, "FreeUnit").map(normalize_id);
        let undeploys_into = ini.get(sec, "UndeploysInto").map(normalize_id);
        let water_bound =
            parse_bool(ini.get(sec, "WaterBound")) || parse_bool(ini.get(sec, "Naval"));
        let detects_cloak =
            parse_bool(ini.get(sec, "Sensors")) || parse_bool(ini.get(sec, "DetectDisguise"));

        let mut script_hooks = BTreeMap::new();
        if parse_bool(ini.get(sec, "SelfHealing")) {
            script_hooks.insert("on_tick".to_string(), "on_self_heal_tick".to_string());
            has_custom_script_hooks = true;
        }

        raw_buildings.push((
            BuildingDef {
                id: id.clone(),
                name,
                faction,
                role,
                cost,
                build_time_secs,
                health,
                armor,
                footprint,
                power_delta,
                vision_range,
                primary_weapon,
                prerequisites: Vec::new(),
                spawns_unit_on_complete: free_unit,
                sub_factions,
                transforms_into_unit: undeploys_into,
                water_bound,
                container: rts_sdk::schema::ContainerSpec {
                    capacity: ini
                        .get(sec, "MaxNumberOccupants")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                    ..Default::default()
                },
                stealth: rts_sdk::schema::StealthSensorSpec {
                    detects_cloak,
                    ..Default::default()
                },
                support_aura: rts_sdk::schema::SupportAuraSpec {
                    cash_trickle_per_sec: if parse_bool(ini.get(sec, "ProduceCashAmount")) {
                        15
                    } else {
                        0
                    },
                    heals_infantry_per_sec: if parse_bool(ini.get(sec, "Hospital")) {
                        5.0
                    } else {
                        0.0
                    },
                    ..Default::default()
                },
                superweapon: None,
                script_hooks,
                visual: VisualDef {
                    model: format!("assets/models/{id}.glb"),
                    icon: None,
                    archetype,
                    scale: [footprint[0] as f32 * 0.9, 1.4, footprint[1] as f32 * 0.9],
                    primary_color,
                    team_color_tint: true,
                    ..Default::default()
                },
                ..BuildingDef::default()
            },
            raw_prereqs,
        ));
    }

    let mut buildings: Vec<BuildingDef> = raw_buildings
        .into_iter()
        .map(|(mut b, raw_prereqs)| {
            b.prerequisites = raw_prereqs
                .into_iter()
                .filter(|p| p != &b.id && known_building_ids.contains(p))
                .collect();
            b
        })
        .collect();

    let mut units = Vec::new();
    let mut known_unit_ids = BTreeSet::new();
    let all_unit_sections: Vec<(String, bool, bool)> = infantry_sections
        .into_iter()
        .map(|s| (s, true, false))
        .chain(vehicle_sections.into_iter().map(|s| {
            let is_inf = infantry_set.contains(&s);
            let is_air = aircraft_set.contains(&s);
            (s, is_inf, is_air)
        }))
        .chain(aircraft_sections.into_iter().map(|s| (s, false, true)))
        .collect();

    for (sec, is_inf, is_air_section) in all_unit_sections {
        if !ini.sections.contains_key(&sec) {
            continue;
        }
        let id = normalize_id(&sec);
        if known_building_ids.contains(&id) || !known_unit_ids.insert(id.clone()) {
            continue;
        }
        let name = ini.get(&sec, "Name").unwrap_or(&sec).to_string();
        let (faction, sub_factions) = map_westwood_faction_and_shared(ini.get(&sec, "Owner"));
        let cost = ini
            .get(&sec, "Cost")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(300);
        let health = ini
            .get(&sec, "Strength")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(200.0)
            .max(25.0);
        let build_time_secs = ((cost as f32) / 100.0).clamp(2.0, 20.0);
        let armor = map_westwood_armor(ini.get(&sec, "Armor"), false);
        let speed = ini
            .get(&sec, "Speed")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|s| (s * 0.8).clamp(2.0, 12.0))
            .unwrap_or(4.5);
        let vision_range = ini
            .get(&sec, "Sight")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|s| (s * 2.5).clamp(10.0, 30.0))
            .unwrap_or(16.0);

        let primary_weapon = ini
            .get(&sec, "Primary")
            .filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case("none"))
            .map(|w| {
                let w_def = extract_weapon_def(w, &ini);
                let w_id = w_def.id.clone();
                weapons_map.entry(w_id.clone()).or_insert(w_def);
                w_id
            });

        let is_aircraft = is_air_section
            || parse_bool(ini.get(&sec, "ConsideredAircraft"))
            || ini
                .get(&sec, "Category")
                .map(|c| c.eq_ignore_ascii_case("AirPower") || c.eq_ignore_ascii_case("AirLift"))
                .unwrap_or(false);
        let is_naval = parse_bool(ini.get(&sec, "Naval"));

        let (role, mut archetype) = infer_unit_role_and_archetype(&sec, &ini, is_inf);
        if is_aircraft {
            archetype = VisualArchetype::Aircraft;
        } else if is_naval {
            archetype = VisualArchetype::Ship;
        }
        let harvest_capacity = if role == UnitRole::Harvester { 500 } else { 0 };
        let prerequisites: Vec<String> = parse_csv_ids(ini.get(&sec, "Prerequisite"))
            .into_iter()
            .filter(|p| known_building_ids.contains(p))
            .collect();

        let (_, primary_color, _) = faction_display_meta(&faction);
        let scale = match archetype {
            VisualArchetype::Infantry => [0.9, 1.0, 0.9],
            VisualArchetype::Harvester => [1.9, 1.3, 2.5],
            VisualArchetype::Aircraft => [2.1, 1.1, 2.6],
            VisualArchetype::Ship => [2.0, 1.2, 3.0],
            _ => [1.8, 1.1, 2.4],
        };

        let secondary_weapon = ini
            .get(&sec, "Secondary")
            .filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case("none"))
            .map(|w| {
                let w_def = extract_weapon_def(w, &ini);
                let w_id = w_def.id.clone();
                weapons_map.entry(w_id.clone()).or_insert(w_def);
                w_id
            });
        let elite_weapon = ini
            .get(&sec, "ElitePrimary")
            .filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case("none"))
            .map(|w| {
                let w_def = extract_weapon_def(w, &ini);
                let w_id = w_def.id.clone();
                weapons_map.entry(w_id.clone()).or_insert(w_def);
                w_id
            });
        let deploys_into = ini
            .get(&sec, "DeploysInto")
            .map(normalize_id)
            .filter(|b| known_building_ids.contains(b));
        let movement_domain = if parse_bool(ini.get(&sec, "JumpJet")) {
            rts_sdk::schema::MovementDomain::Jumpjet
        } else if is_aircraft {
            rts_sdk::schema::MovementDomain::Air
        } else if parse_bool(ini.get(&sec, "Teleporter")) {
            rts_sdk::schema::MovementDomain::Chrono
        } else if is_naval {
            rts_sdk::schema::MovementDomain::Naval
        } else {
            rts_sdk::schema::MovementDomain::Ground
        };

        let mut script_hooks = BTreeMap::new();
        if parse_bool(ini.get(&sec, "SelfHealing")) {
            script_hooks.insert("on_tick".to_string(), "on_self_heal_tick".to_string());
            has_custom_script_hooks = true;
        }

        units.push(UnitDef {
            id: id.clone(),
            name,
            faction,
            role,
            cost,
            build_time_secs,
            health,
            armor,
            speed,
            vision_range,
            harvest_capacity,
            primary_weapon,
            secondary_weapon,
            elite_weapon,
            deploy_weapon: None,
            prerequisites,
            sub_factions,
            movement_domain,
            cruise_altitude: match movement_domain {
                rts_sdk::schema::MovementDomain::Air => 4.2,
                rts_sdk::schema::MovementDomain::Jumpjet => 3.2,
                _ => 0.0,
            },
            transforms_into_building: deploys_into,
            immobile_when_deployed: parse_bool(ini.get(&sec, "Deployer")),
            chrono_harvester: id == "cmin" || parse_bool(ini.get(&sec, "Teleporter")),
            spy_infiltrator: parse_bool(ini.get(&sec, "Infiltrate"))
                || parse_bool(ini.get(&sec, "Agent")),
            mind_control_limit: if id.contains("yuri") { 1 } else { 0 },
            crushes_infantry: parse_bool(ini.get(&sec, "Crusher"))
                || (role == UnitRole::Vehicle && !is_aircraft && !is_naval),
            ability_id: None,
            container: rts_sdk::schema::ContainerSpec {
                capacity: ini
                    .get(&sec, "Passengers")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                ..Default::default()
            },
            stealth: rts_sdk::schema::StealthSensorSpec {
                cloaked: parse_bool(ini.get(&sec, "Cloakable")),
                ..Default::default()
            },
            script_hooks,
            visual: VisualDef {
                model: format!("assets/models/{id}.glb"),
                icon: None,
                archetype,
                scale,
                primary_color,
                team_color_tint: true,
                ..Default::default()
            },
            ..UnitDef::default()
        });
    }

    if buildings.is_empty() && units.is_empty() {
        bail!("No valid Westwood buildings or units found in INI input");
    }

    // Link Refinery buildings to a valid Harvester unit if spawns_unit_on_complete is not set or invalid
    for b in &mut buildings {
        let valid_existing = b
            .spawns_unit_on_complete
            .as_ref()
            .map(|uid| known_unit_ids.contains(uid))
            .unwrap_or(false);
        if !valid_existing {
            b.spawns_unit_on_complete = None;
        }
        if let Some(ref tu) = b.transforms_into_unit {
            if !known_unit_ids.contains(tu) {
                b.transforms_into_unit = None;
            }
        }
        if b.role == BuildingRole::Refinery && b.spawns_unit_on_complete.is_none() {
            b.spawns_unit_on_complete = units
                .iter()
                .find(|u| u.role == UnitRole::Harvester && u.is_available_to_faction(&b.faction))
                .or_else(|| units.iter().find(|u| u.role == UnitRole::Harvester))
                .map(|u| u.id.clone());
        }
    }

    let factions = build_dynamic_faction_catalog(&buildings, &units);

    let map = parse_westwood_map_ini(&ini, &buildings, &units, &factions)
        .unwrap_or_else(|| build_default_converted_map(&buildings, &units));

    let scripts = if has_custom_script_hooks {
        vec!["scripts/auto_traits.rhai".to_string()]
    } else {
        Vec::new()
    };

    let manifest = ModManifest {
        id: mod_id.to_string(),
        name: mod_name.to_string(),
        version: "0.1.0".to_string(),
        extends: None,
        authors: vec!["rts-mod-tools Westwood Converter".to_string()],
        description: "Converted from legacy Westwood INI, VXL & SHP data".to_string(),
        engine_version: "0.1.0".to_string(),
        entry_map: "maps/converted_skirmish.toml".to_string(),
        catalogs: ModCatalogPaths {
            factions: "factions/factions.toml".to_string(),
            weapons: "weapons/weapons.toml".to_string(),
            buildings: "buildings/buildings.toml".to_string(),
            units: "units/units.toml".to_string(),
        },
        scripts,
        rules: Default::default(),
    };

    Ok(ConvertedModBundle {
        manifest,
        factions,
        weapons: WeaponCatalog {
            weapons: weapons_map.into_values().collect(),
        },
        buildings: BuildingCatalog { buildings },
        units: UnitCatalog { units },
        map,
    })
}

/// Parses a Westwood `.map` / `.mpr` INI section (`[Map]`, `[Waypoints]`, `[OverlayPack]`) into a `MapDef` if present.
pub fn parse_westwood_map_ini(
    ini: &WestwoodIni,
    buildings: &[BuildingDef],
    units: &[UnitDef],
    factions: &FactionCatalog,
) -> Option<MapDef> {
    let size_str = ini
        .get("Map", "Size")
        .or_else(|| ini.get("Map", "LocalSize"))?;
    let parts: Vec<u32> = size_str
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect();
    if parts.len() < 4 {
        return None;
    }
    let width = parts[2].clamp(24, 128);
    let height = parts[3].clamp(24, 128);

    let map_name = ini
        .get("Basic", "Name")
        .unwrap_or("Converted Westwood Map")
        .to_string();

    let mut base_map = build_default_converted_map(buildings, units);
    base_map.name = map_name;
    base_map.width = width;
    base_map.height = height;

    // Extract waypoints (0..7 are multiplayer start positions in Westwood maps)
    let mut wp_coords: Vec<[i32; 2]> = Vec::new();
    let max_x = width as i32 - 4;
    let max_y = height as i32 - 4;
    for wp_idx in 0..8 {
        if let Some(val) = ini.get("Waypoints", &wp_idx.to_string()) {
            if let Ok(cell) = val.trim().parse::<i32>() {
                // Classic RA/TS/RA2 cell encoding: cell = y * 1000 + x or y * 128 + x
                let (wx, wy) = if cell >= 1000 {
                    ((cell % 1000).min(max_x), (cell / 1000).min(max_y))
                } else {
                    ((cell % 128).min(max_x), (cell / 128).min(max_y))
                };
                wp_coords.push([wx.max(4), wy.max(4)]);
            }
        }
    }
    if wp_coords.len() >= 2 {
        for (idx, spawn) in base_map.spawns.iter_mut().enumerate() {
            if let Some(&coord) = wp_coords.get(idx) {
                spawn.base_tile = coord;
                if let Some(b) = spawn.initial_buildings.first_mut() {
                    b.tile_pos = coord;
                }
                for (u_idx, u) in spawn.initial_units.iter_mut().enumerate() {
                    u.tile_pos = [
                        (coord[0] + 3 + (u_idx as i32 % 3)).min(width as i32 - 2),
                        (coord[1] + 1 + (u_idx as i32 / 3)).min(height as i32 - 2),
                    ];
                }
            }
            if let Some(f) = factions.factions.get(idx % factions.factions.len().max(1)) {
                spawn.faction = f.id.clone();
            }
        }
        base_map.ore_patches = wp_coords
            .iter()
            .take(2)
            .map(|c| OrePatchDef {
                position: [
                    (c[0] + 5).min(width as i32 - 3),
                    (c[1] + 4).min(height as i32 - 3),
                ],
                radius: 3,
                amount_per_tile: 650,
            })
            .collect();
    }

    Some(base_map)
}

