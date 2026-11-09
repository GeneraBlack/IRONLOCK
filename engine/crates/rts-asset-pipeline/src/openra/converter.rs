use anyhow::bail;
use rts_sdk::schema::ProjectileStyle;
use super::map_reader::*;
use super::miniyaml::*;
use crate::westwood::{
    build_default_converted_map, build_dynamic_faction_catalog, faction_display_meta, normalize_id,
    ConvertedModBundle,
};
use anyhow::Result;
use rts_sdk::manifest::{ModCatalogPaths, ModManifest};
use rts_sdk::schema::{
    BuildingCatalog, BuildingDef, BuildingRole, MovementDomain, SuperweaponType, UnitCatalog,
    UnitDef, UnitRole, VisualArchetype, VisualDef, WeaponCatalog, WeaponDef,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn convert_openra_yaml_to_bundle(
    rules_yaml: &str,
    weapons_yaml: Option<&str>,
    map_yaml: Option<&str>,
    mod_id: &str,
    mod_name: &str,
) -> Result<ConvertedModBundle> {
    let raw_nodes = parse_miniyaml(rules_yaml);
    let mut templates = BTreeMap::new();
    for node in &raw_nodes {
        templates.insert(node.key.clone(), node.clone());
    }

    let mut weapons_map: BTreeMap<String, WeaponDef> = BTreeMap::new();
    if let Some(w_str) = weapons_yaml {
        let w_nodes = parse_miniyaml(w_str);
        let mut w_templates = BTreeMap::new();
        for w_node in &w_nodes {
            w_templates.insert(w_node.key.clone(), w_node.clone());
        }
        for w_node in &w_nodes {
            if w_node.key.starts_with('^') {
                continue;
            }
            let merged_w = merge_inherited_traits(w_node, &w_templates, 0);
            let def = parse_openra_weapon_node(&merged_w);
            weapons_map.insert(def.id.clone(), def);
        }
    }

    let resolved_actors: Vec<MiniYamlNode> = raw_nodes
        .iter()
        .filter(|n| !n.key.starts_with('^'))
        .map(|n| merge_inherited_traits(n, &templates, 0))
        .collect();

    let has_buildable_actors = resolved_actors
        .iter()
        .any(|a| a.child("Buildable").is_some() || a.child("BaseProvider").is_some());

    let mut raw_buildings = Vec::new();
    let mut raw_units = Vec::new();
    let mut known_building_ids = BTreeSet::new();
    let mut provided_by_faction: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut provided_any: BTreeMap<String, String> = BTreeMap::new();
    let mut has_custom_script_hooks = false;

    for actor in &resolved_actors {
        let Some(health_raw) = actor.nested_value("Health", "HP") else {
            continue;
        };
        let Ok(raw_hp) = health_raw.parse::<f32>() else {
            continue;
        };

        if has_buildable_actors {
            let is_base_provider = actor.child("BaseProvider").is_some();
            let is_tech_structure = actor.child("CashTrickler").is_some()
                || matches!(
                    actor.key.to_ascii_lowercase().as_str(),
                    "caoild" | "cahosp" | "caairp" | "caoutp"
                );
            let is_enabled_buildable = actor.child("Buildable").is_some()
                && !actor
                    .nested_value("Buildable", "Prerequisites")
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("~disabled");
            if !is_base_provider && !is_enabled_buildable && !is_tech_structure {
                continue;
            }
        }

        let health = if raw_hp >= 5000.0 {
            (raw_hp / 100.0).max(40.0)
        } else {
            raw_hp.max(40.0)
        };

        let id = normalize_id(&actor.key);
        if id.contains("colorpicker") {
            continue;
        }
        let name = actor
            .nested_value("Tooltip", "Name")
            .or_else(|| actor.nested_value("DisguiseTooltip", "Name"))
            .unwrap_or(&actor.key)
            .to_string();
        let faction = infer_openra_faction(actor);
        let sub_factions = extract_openra_sub_factions(actor);
        let cost = actor
            .nested_value("Valued", "Cost")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(400);
        let build_time_secs = ((cost as f32) / 100.0).clamp(2.5, 25.0);
        let vision_range = actor
            .nested_value("RevealsShroud", "Range")
            .and_then(parse_openra_range)
            .unwrap_or(16.0);

        let mut primary_weapon: Option<String> = None;
        let mut secondary_weapon: Option<String> = None;
        let mut elite_weapon: Option<String> = None;
        let mut deploy_weapon: Option<String> = None;

        for arm in actor.children_matching("Armament") {
            let Some(w_raw) = arm.child_value("Weapon") else {
                continue;
            };
            let w_id = normalize_id(w_raw);
            weapons_map.entry(w_id.clone()).or_insert_with(|| WeaponDef {
                id: w_id.clone(),
                name: w_raw.to_string(),
                damage: 45.0,
                range: 14.0,
                cooldown_secs: 1.5,
                projectile_speed: 35.0,
                armor_multipliers: BTreeMap::new(),
                ..Default::default()
            });
            let arm_key_lower = arm.key.to_ascii_lowercase();
            let req_cond = arm
                .child_value("RequiresCondition")
                .unwrap_or("")
                .to_ascii_lowercase();
            if arm_key_lower.contains("elite")
                || (req_cond.contains("rank-elite") && !req_cond.contains("!rank-elite"))
            {
                elite_weapon = Some(w_id);
            } else if arm_key_lower.contains("deployed")
                || (req_cond.contains("deployed") && !req_cond.contains("!deployed"))
            {
                deploy_weapon = Some(w_id);
            } else if primary_weapon.is_none() {
                primary_weapon = Some(w_id);
            } else if secondary_weapon.is_none() && primary_weapon.as_ref() != Some(&w_id) {
                secondary_weapon = Some(w_id);
            }
        }
        if elite_weapon.is_none() {
            if let Some(ref pw) = primary_weapon {
                let candidate = format!("{pw}e");
                if weapons_map.contains_key(&candidate) {
                    elite_weapon = Some(candidate);
                }
            }
        }
        if primary_weapon.is_none() && actor.child("MindController").is_some() {
            let mc_id = "mind_control_psi".to_string();
            weapons_map.entry(mc_id.clone()).or_insert_with(|| WeaponDef {
                id: mc_id.clone(),
                name: "Psychic Mind Control".to_string(),
                damage: 10.0,
                range: 16.0,
                cooldown_secs: 3.5,
                projectile_speed: 40.0,
                projectile_style: ProjectileStyle::MindControl,
                mind_control: true,
                ..Default::default()
            });
            primary_weapon = Some(mc_id);
        }

        let transforms_into = actor
            .nested_value("Transforms", "IntoActor")
            .map(normalize_id);

        let is_building = actor.child("Building").is_some()
            || actor.child("Power").is_some()
            || actor.child("Production").is_some()
            || actor.child("Refinery").is_some()
            || actor.child("BaseProvider").is_some()
            || actor.child("CashTrickler").is_some()
            || actor
                .children_matching("Inherits")
                .iter()
                .any(|i| {
                    let v = i.value.to_ascii_lowercase();
                    v.contains("building") || v.contains("defense")
                });

        let prereq_raw = actor
            .nested_value("Buildable", "Prerequisites")
            .map(String::from);

        if is_building {
            if !known_building_ids.insert(id.clone()) {
                continue;
            }
            for prov in actor.children_matching("ProvidesPrerequisite") {
                if let Some(p_tok) = prov.child_value("Prerequisite") {
                    let norm_tok = normalize_id(p_tok);
                    provided_by_faction
                        .entry((faction.clone(), norm_tok.clone()))
                        .or_insert_with(|| id.clone());
                    provided_any
                        .entry(norm_tok)
                        .or_insert_with(|| id.clone());
                }
            }
            let free_actor = actor
                .nested_value("FreeActor", "Actor")
                .map(normalize_id);
            let armor = map_openra_armor(actor.nested_value("Armor", "Type"), true);
            let power_delta = actor
                .nested_value("Power", "Amount")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);

            let id_lower = id.to_ascii_lowercase();
            let produces = actor
                .nested_value("Production", "Produces")
                .unwrap_or("")
                .to_ascii_lowercase();

            let is_conyard_id = id_lower.contains("cnst")
                || id_lower == "fact"
                || id_lower == "sfact"
                || id_lower.contains("conyard");
            let is_war_factory = produces.contains("vehicle")
                || id_lower.contains("weap")
                || id_lower.contains("war_factory");
            let is_barracks = produces.contains("infantry")
                || id_lower.contains("barr")
                || id_lower.contains("tent")
                || id_lower.contains("pile")
                || id_lower.contains("hand");

            let (role, archetype) = if is_conyard_id
                || (actor.child("BaseProvider").is_some()
                    && !is_war_factory
                    && !is_barracks
                    && !id_lower.contains("yard"))
            {
                (
                    BuildingRole::ConstructionYard,
                    VisualArchetype::ConstructionYard,
                )
            } else if power_delta > 0
                || id_lower.contains("powr")
                || id_lower.contains("apwr")
                || id_lower.contains("nrct")
            {
                (BuildingRole::PowerPlant, VisualArchetype::PowerPlant)
            } else if actor.child("Refinery").is_some()
                || id_lower.contains("proc")
                || id_lower.contains("refn")
            {
                (BuildingRole::Refinery, VisualArchetype::Refinery)
            } else if is_barracks {
                (BuildingRole::Barracks, VisualArchetype::Barracks)
            } else if is_war_factory {
                (BuildingRole::WarFactory, VisualArchetype::WarFactory)
            } else if primary_weapon.is_some()
                || id_lower.contains("tsla")
                || id_lower.contains("tesla")
                || id_lower.contains("pbox")
                || id_lower.contains("pill")
                || id_lower.contains("lasr")
                || id_lower.contains("flak")
                || id_lower.contains("sam")
                || id_lower.contains("gcan")
                || id_lower.contains("gun")
            {
                (BuildingRole::Defense, VisualArchetype::Turret)
            } else if id_lower.contains("atek")
                || id_lower.contains("stek")
                || id_lower.contains("tech")
            {
                (BuildingRole::TechCenter, VisualArchetype::GenericBox)
            } else {
                (BuildingRole::Support, VisualArchetype::GenericBox)
            };

            let footprint = actor
                .nested_value("Building", "Dimensions")
                .and_then(|dim| {
                    let (w, h) = dim.split_once(',')?;
                    Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
                })
                .unwrap_or([2, 2]);

            let water_bound = actor
                .nested_value("Building", "TerrainTypes")
                .map(|t| t.to_ascii_lowercase().contains("water"))
                .unwrap_or(false)
                || id_lower.contains("yard");
            let detects_cloak = actor.child("DetectCloaked").is_some();
            let cash_trickle_per_sec = actor
                .nested_value("CashTrickler", "Amount")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let heals_infantry_per_sec = if id_lower == "cahosp" { 6.0 } else { 0.0 };
            let garrison_capacity = actor
                .nested_value("Cargo", "MaxWeight")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let superweapon = if actor.child("NukePower").is_some() || id_lower == "namisl" {
                Some(SuperweaponType::NuclearMissile)
            } else if actor.child("WeatherControlSupportPower").is_some() || id_lower == "gaweat" {
                Some(SuperweaponType::WeatherStorm)
            } else if id_lower == "nairon" || id_lower == "iron" {
                Some(SuperweaponType::IronCurtain)
            } else if actor.child("ChronoshiftPower").is_some() || id_lower == "gacsph" {
                Some(SuperweaponType::Chronosphere)
            } else if actor.child("ParatroopersPower").is_some() {
                Some(SuperweaponType::Paratroopers)
            } else {
                None
            };

            let primary_color = faction_display_meta(&faction).1;
            let mut script_hooks = BTreeMap::new();
            if actor.child("ChangesHealth").is_some() || actor.child("SelfHealing").is_some() {
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
                    spawns_unit_on_complete: free_actor,
                    sub_factions,
                    transforms_into_unit: transforms_into,
                    water_bound,
                    container: rts_sdk::schema::ContainerSpec {
                        capacity: garrison_capacity,
                        ..Default::default()
                    },
                    stealth: rts_sdk::schema::StealthSensorSpec {
                        detects_cloak,
                        ..Default::default()
                    },
                    support_aura: rts_sdk::schema::SupportAuraSpec {
                        cash_trickle_per_sec,
                        heals_infantry_per_sec,
                        ..Default::default()
                    },
                    superweapon,
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
                prereq_raw,
            ));
        } else {
            let armor = map_openra_armor(actor.nested_value("Armor", "Type"), false);
            let raw_speed = actor
                .nested_value("Mobile", "Speed")
                .or_else(|| actor.nested_value("Aircraft", "Speed"))
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(60.0);
            let speed = if raw_speed > 15.0 {
                (raw_speed / 14.0).clamp(2.2, 12.0)
            } else {
                raw_speed.clamp(2.2, 12.0)
            };

            let locomotor = actor
                .nested_value("Mobile", "Locomotor")
                .unwrap_or("")
                .to_ascii_lowercase();
            let is_aircraft = actor.child("Aircraft").is_some()
                || actor
                    .children_matching("Inherits")
                    .iter()
                    .any(|i| i.value.to_ascii_lowercase().contains("plane") || i.value.to_ascii_lowercase().contains("aircraft"));
            let movement_domain = if locomotor.contains("jumpjet") || id == "jumpjet" {
                MovementDomain::Jumpjet
            } else if is_aircraft {
                MovementDomain::Air
            } else if actor.child("PortableChrono").is_some()
                || locomotor.contains("chrono")
                || locomotor.contains("teleport")
                || id == "chrono"
            {
                MovementDomain::Chrono
            } else if locomotor.contains("naval") {
                MovementDomain::Naval
            } else if locomotor.contains("amphib")
                || locomotor.contains("hover")
                || locomotor.contains("lcrf")
                || id == "sapc"
                || id == "lcrf"
            {
                MovementDomain::Amphibious
            } else {
                MovementDomain::Ground
            };

            let cruise_altitude = match movement_domain {
                MovementDomain::Air => 4.2,
                MovementDomain::Jumpjet => 3.0,
                _ => 0.0,
            };

            let is_infantry = actor
                .children_matching("Inherits")
                .iter()
                .any(|i| {
                    let v = i.value.to_ascii_lowercase();
                    v.contains("soldier") || v.contains("infantry")
                })
                || actor.child("TakeCover").is_some()
                || id == "jumpjet";

            let (role, archetype) = if actor.child("Harvester").is_some()
                || id.contains("harv")
                || id.contains("miner")
            {
                (UnitRole::Harvester, VisualArchetype::Harvester)
            } else if actor.child("Captures").is_some()
                || actor.child("InstantlyRepairs").is_some()
                || id == "e6"
                || id.contains("engi")
            {
                (UnitRole::Engineer, VisualArchetype::Infantry)
            } else if is_infantry {
                (UnitRole::Infantry, VisualArchetype::Infantry)
            } else if is_aircraft {
                (UnitRole::Vehicle, VisualArchetype::Aircraft)
            } else if movement_domain == MovementDomain::Naval {
                (UnitRole::Vehicle, VisualArchetype::Ship)
            } else {
                (UnitRole::Vehicle, VisualArchetype::Tank)
            };

            let harvest_capacity = if role == UnitRole::Harvester { 500 } else { 0 };
            let immobile_when_deployed = actor.child("GrantConditionOnDeploy").is_some()
                || deploy_weapon.is_some();
            let chrono_harvester = actor.child("ChronoResourceDelivery").is_some() || id == "cmin";
            let cloaked = actor.child("Cloak").is_some() || actor.child("Mirage").is_some();
            let spy_infiltrator = actor.child("Infiltrates").is_some()
                || actor.child("Disguise").is_some()
                || id == "spy";
            let mind_control_limit = if actor.child("MindController").is_some() {
                actor
                    .nested_value("MindController", "Capacity")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(1)
            } else {
                0
            };
            let passenger_slots = actor
                .nested_value("Cargo", "MaxWeight")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let crushes_infantry = actor
                .nested_value("Mobile", "Crushes")
                .map(|c| c.to_ascii_lowercase().contains("infantry"))
                .unwrap_or(false)
                || locomotor.contains("tracked");

            let ability_id = if transforms_into.is_some() {
                Some("transform_mcv".to_string())
            } else if immobile_when_deployed {
                Some("deploy_stance".to_string())
            } else if movement_domain == MovementDomain::Chrono {
                Some("chronoshift".to_string())
            } else {
                None
            };

            let primary_color = faction_display_meta(&faction).1;
            let mut script_hooks = BTreeMap::new();
            if actor.child("ChangesHealth").is_some() || actor.child("SelfHealing").is_some() {
                script_hooks.insert("on_tick".to_string(), "on_self_heal_tick".to_string());
                has_custom_script_hooks = true;
            }
            let scale = match archetype {
                VisualArchetype::Infantry => [0.9, 1.0, 0.9],
                VisualArchetype::Harvester => [1.9, 1.3, 2.5],
                VisualArchetype::Aircraft => [2.1, 1.1, 2.6],
                VisualArchetype::Ship => [2.0, 1.2, 3.0],
                _ => [1.8, 1.1, 2.4],
            };

            raw_units.push((
                UnitDef {
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
                    deploy_weapon,
                    prerequisites: Vec::new(),
                    sub_factions,
                    movement_domain,
                    cruise_altitude,
                    transforms_into_building: transforms_into,
                    immobile_when_deployed,
                    chrono_harvester,
                    spy_infiltrator,
                    mind_control_limit,
                    crushes_infantry,
                    ability_id,
                    container: rts_sdk::schema::ContainerSpec {
                        capacity: passenger_slots,
                        ..Default::default()
                    },
                    stealth: rts_sdk::schema::StealthSensorSpec {
                        cloaked,
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
                },
                prereq_raw,
            ));
        }
    }

    if raw_buildings.is_empty() && raw_units.is_empty() {
        bail!("No valid OpenRA actors found in MiniYAML input");
    }

    let mut buildings: Vec<BuildingDef> = raw_buildings
        .into_iter()
        .map(|(mut b, prereq_raw)| {
            b.prerequisites = clean_openra_prerequisites(
                prereq_raw.as_deref(),
                &b.faction,
                &known_building_ids,
                &provided_by_faction,
                &provided_any,
            )
            .into_iter()
            .filter(|p| p != &b.id)
            .collect();
            b
        })
        .collect();

    let units: Vec<UnitDef> = raw_units
        .into_iter()
        .map(|(mut u, prereq_raw)| {
            u.prerequisites = clean_openra_prerequisites(
                prereq_raw.as_deref(),
                &u.faction,
                &known_building_ids,
                &provided_by_faction,
                &provided_any,
            );
            if let Some(ref tb) = u.transforms_into_building {
                if !known_building_ids.contains(tb) {
                    u.transforms_into_building = None;
                }
            }
            u
        })
        .collect();

    let known_unit_ids: BTreeSet<String> = units.iter().map(|u| u.id.clone()).collect();
    for b in &mut buildings {
        if let Some(ref spawn_id) = b.spawns_unit_on_complete {
            if !known_unit_ids.contains(spawn_id) {
                b.spawns_unit_on_complete = None;
            }
        }
        if let Some(ref trans_u) = b.transforms_into_unit {
            if !known_unit_ids.contains(trans_u) {
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

    let map = if let Some(m_yaml) = map_yaml {
        convert_openra_map_yaml(m_yaml, &buildings, &units)
    } else {
        build_default_converted_map(&buildings, &units)
    };

    let factions = build_dynamic_faction_catalog(&buildings, &units);

    let scripts = if has_custom_script_hooks {
        vec!["scripts/auto_traits.rhai".to_string()]
    } else {
        Vec::new()
    };

    let entry_map = format!("maps/{}.toml", map.id);
    let manifest = ModManifest {
        id: mod_id.to_string(),
        name: mod_name.to_string(),
        version: "0.1.0".to_string(),
        extends: None,
        authors: vec!["rts-mod-tools OpenRA Converter".to_string()],
        description: "Converted from OpenRA MiniYAML rules, weapons, and map".to_string(),
        engine_version: "0.1.0".to_string(),
        entry_map,
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

