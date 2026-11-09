use rts_sdk::schema::{
    ArmorType, BuildingDef, BuildingRole, FactionCatalog, FactionDef, UnitDef, UnitRole, VisualArchetype, WeaponDef,
};
use std::collections::{BTreeMap, BTreeSet};

/// Parsed representation of a Westwood `.ini` file (`rules.ini`, `rulesmd.ini`, `art.ini`).
#[derive(Debug, Clone, Default)]
pub struct WestwoodIni {
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
    pub section_order: Vec<String>,
}

impl WestwoodIni {
    pub fn parse(input: &str) -> Self {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut section_order = Vec::new();
        let mut current_section: Option<String> = None;

        for raw_line in input.lines() {
            let line = raw_line.split(';').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
                let sec = line[1..line.len() - 1].trim().to_string();
                if !sections.contains_key(&sec) {
                    section_order.push(sec.clone());
                    sections.insert(sec.clone(), BTreeMap::new());
                }
                current_section = Some(sec);
                continue;
            }
            if let Some(sec) = &current_section {
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim().to_string();
                    let val = v.trim().to_string();
                    if let Some(map) = sections.get_mut(sec) {
                        map.insert(key, val);
                    }
                }
            }
        }

        Self {
            sections,
            section_order,
        }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let sec_map = self.sections.get(section).or_else(|| {
            self.sections
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(section))
                .map(|(_, v)| v)
        })?;
        sec_map
            .get(key)
            .or_else(|| {
                sec_map
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v)
            })
            .map(String::as_str)
    }

    pub fn list_values(&self, section: &str) -> Vec<String> {
        let Some(sec_map) = self.sections.get(section).or_else(|| {
            self.sections
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(section))
                .map(|(_, v)| v)
        }) else {
            return Vec::new();
        };
        let mut out: Vec<String> = sec_map
            .values()
            .filter(|v| !v.is_empty())
            .cloned()
            .collect();
        out.dedup();
        out
    }
}

pub fn normalize_id(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '_' || ch == '-' || ch == ' ') && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "unnamed".to_string()
    } else if out.chars().next().unwrap().is_ascii_digit() {
        format!("id_{out}")
    } else {
        out
    }
}

pub fn faction_display_meta(faction_id: &str) -> (&'static str, [f32; 3], &'static str) {
    match faction_id {
        "soviets" => (
            "Soviet Union",
            [0.78, 0.18, 0.18],
            "Heavy armor, Tesla weapons, and nuclear superiority",
        ),
        "yuri" => (
            "Yuri's Psychic Corps",
            [0.64, 0.18, 0.82],
            "Psychic mind control, cloning, and genetic warfare",
        ),
        "gdi" => (
            "Global Defense Initiative",
            [0.86, 0.72, 0.22],
            "High-tech conventional military and Ion Cannon support",
        ),
        "nod" => (
            "Brotherhood of Nod",
            [0.82, 0.14, 0.14],
            "Stealth, laser technology, and Tiberium weaponry",
        ),
        "atreides" => (
            "House Atreides",
            [0.24, 0.52, 0.88],
            "Noble house of Arrakis with sonic weaponry and air power",
        ),
        "harkonnen" => (
            "House Harkonnen",
            [0.80, 0.16, 0.16],
            "Brutal heavy assault forces and Devastator armor",
        ),
        "ordos" => (
            "House Ordos",
            [0.20, 0.76, 0.42],
            "Insidious mercenary cartel with deviator gas and saboteurs",
        ),
        "scrin" => (
            "Scrin Invaders",
            [0.52, 0.22, 0.88],
            "Extraterrestrial Tiberium-based fleet",
        ),
        _ => (
            "Allied Nations",
            [0.22, 0.45, 0.82],
            "High-tech Chrono, Prism, and air superiority forces",
        ),
    }
}

pub fn build_dynamic_faction_catalog(
    buildings: &[BuildingDef],
    units: &[UnitDef],
) -> FactionCatalog {
    let mut seen = BTreeSet::new();
    for b in buildings {
        seen.insert(b.faction.clone());
    }
    for u in units {
        seen.insert(u.faction.clone());
    }
    if seen.is_empty() {
        seen.insert("allies".to_string());
        seen.insert("soviets".to_string());
    } else if seen.len() == 1 {
        if seen.contains("allies") {
            seen.insert("soviets".to_string());
        } else if seen.contains("gdi") {
            seen.insert("nod".to_string());
        } else if seen.contains("atreides") {
            seen.insert("harkonnen".to_string());
        } else {
            seen.insert("allies".to_string());
        }
    }

    let factions = seen
        .into_iter()
        .map(|fid| {
            let (name, color, desc) = faction_display_meta(&fid);
            FactionDef {
                id: fid,
                name: name.to_string(),
                color,
                starting_credits: 10000,
                description: desc.to_string(),
                ..Default::default()
            }
        })
        .collect();
    FactionCatalog { factions }
}

pub(super) fn map_westwood_faction_and_shared(owner_raw: Option<&str>) -> (String, Vec<String>) {
    let Some(owner) = owner_raw else {
        return ("allies".to_string(), Vec::new());
    };
    let lower = owner.to_ascii_lowercase();
    let mut major = Vec::new();
    let mut subs = Vec::new();

    if lower.contains("allies")
        || lower.contains("british")
        || lower.contains("french")
        || lower.contains("german")
        || lower.contains("american")
        || lower.contains("korea")
        || lower.contains("goodguy")
    {
        major.push("allies".to_string());
    }
    if lower.contains("soviet")
        || lower.contains("russian")
        || lower.contains("iraq")
        || lower.contains("cuba")
        || lower.contains("libya")
        || lower.contains("ukraine")
        || lower.contains("badguy")
    {
        major.push("soviets".to_string());
    }
    if lower.contains("yuri") || lower.contains("thirdside") || lower.contains("psychic") {
        major.push("yuri".to_string());
    }
    if lower.contains("gdi") {
        major.push("gdi".to_string());
    }
    if lower.contains("nod") {
        major.push("nod".to_string());
    }
    if lower.contains("atreides") {
        major.push("atreides".to_string());
    }
    if lower.contains("harkonnen") {
        major.push("harkonnen".to_string());
    }
    if lower.contains("ordos") {
        major.push("ordos".to_string());
    }
    if lower.contains("scrin") || lower.contains("mutant") {
        major.push("scrin".to_string());
    }

    for &(tok, norm) in &[
        ("american", "america"),
        ("british", "england"),
        ("french", "france"),
        ("german", "germany"),
        ("korea", "korea"),
        ("russian", "russia"),
        ("iraq", "iraq"),
        ("libya", "libya"),
        ("cuba", "cuba"),
    ] {
        if lower.contains(tok) {
            subs.push(norm.to_string());
        }
    }

    if major.is_empty() {
        major.push("allies".to_string());
    }
    let primary = major.remove(0);
    for m in major {
        if !subs.contains(&m) {
            subs.push(m);
        }
    }
    (primary, subs)
}

pub(super) fn map_westwood_armor(raw: Option<&str>, is_building: bool) -> ArmorType {
    let Some(armor) = raw else {
        return if is_building {
            ArmorType::Structure
        } else {
            ArmorType::Light
        };
    };
    match armor.trim().to_ascii_lowercase().as_str() {
        "none" => ArmorType::None,
        "flak" | "plate" | "light" => ArmorType::Light,
        "medium" => ArmorType::Medium,
        "heavy" => ArmorType::Heavy,
        "wood" | "steel" | "concrete" | "structure" => ArmorType::Structure,
        _ => {
            if is_building {
                ArmorType::Structure
            } else {
                ArmorType::Light
            }
        }
    }
}

pub(super) fn parse_csv_ids(raw: Option<&str>) -> Vec<String> {
    let Some(s) = raw else {
        return Vec::new();
    };
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case("none"))
        .map(normalize_id)
        .collect()
}

pub(super) fn parse_bool(raw: Option<&str>) -> bool {
    let Some(s) = raw else {
        return false;
    };
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "1"
    )
}

pub(super) fn infer_building_role_and_archetype(
    sec_name: &str,
    ini: &WestwoodIni,
    power_delta: i32,
    has_weapon: bool,
) -> (BuildingRole, VisualArchetype) {
    let id_lower = sec_name.to_ascii_lowercase();
    let factory = ini
        .get(sec_name, "Factory")
        .unwrap_or("")
        .to_ascii_lowercase();

    if parse_bool(ini.get(sec_name, "ConstructionYard"))
        || factory.contains("building")
        || id_lower.contains("fact")
        || id_lower.contains("conyard")
    {
        (
            BuildingRole::ConstructionYard,
            VisualArchetype::ConstructionYard,
        )
    } else if power_delta > 0 || id_lower.contains("powr") || id_lower.contains("apwr") {
        (BuildingRole::PowerPlant, VisualArchetype::PowerPlant)
    } else if parse_bool(ini.get(sec_name, "Refinery"))
        || id_lower.contains("proc")
        || id_lower.contains("ref")
    {
        (BuildingRole::Refinery, VisualArchetype::Refinery)
    } else if factory.contains("infantry")
        || id_lower.contains("barr")
        || id_lower.contains("pile")
        || id_lower.contains("hand")
        || id_lower.contains("tent")
    {
        (BuildingRole::Barracks, VisualArchetype::Barracks)
    } else if factory.contains("unit")
        || parse_bool(ini.get(sec_name, "WeaponsFactory"))
        || id_lower.contains("weap")
    {
        (BuildingRole::WarFactory, VisualArchetype::WarFactory)
    } else if has_weapon
        || parse_bool(ini.get(sec_name, "IsBaseDefense"))
        || id_lower.contains("tsla")
        || id_lower.contains("pbox")
        || id_lower.contains("gun")
    {
        (BuildingRole::Defense, VisualArchetype::Turret)
    } else if id_lower.contains("tech") || id_lower.contains("lab") {
        (BuildingRole::TechCenter, VisualArchetype::GenericBox)
    } else {
        (BuildingRole::Support, VisualArchetype::GenericBox)
    }
}

pub(super) fn infer_unit_role_and_archetype(
    sec_name: &str,
    ini: &WestwoodIni,
    is_infantry_list: bool,
) -> (UnitRole, VisualArchetype) {
    let id_lower = sec_name.to_ascii_lowercase();
    let category = ini
        .get(sec_name, "Category")
        .unwrap_or("")
        .to_ascii_lowercase();

    if parse_bool(ini.get(sec_name, "Harvester"))
        || parse_bool(ini.get(sec_name, "ResourceGatherer"))
        || id_lower.contains("harv")
        || id_lower.contains("miner")
    {
        (UnitRole::Harvester, VisualArchetype::Harvester)
    } else if parse_bool(ini.get(sec_name, "Engineer"))
        || id_lower.contains("engi")
        || id_lower == "e6"
    {
        (UnitRole::Engineer, VisualArchetype::Infantry)
    } else if is_infantry_list
        || category.contains("soldier")
        || parse_bool(ini.get(sec_name, "Crushable"))
    {
        (UnitRole::Infantry, VisualArchetype::Infantry)
    } else {
        (UnitRole::Vehicle, VisualArchetype::Tank)
    }
}

pub(super) fn extract_weapon_def(weapon_raw_name: &str, ini: &WestwoodIni) -> WeaponDef {
    let id = normalize_id(weapon_raw_name);
    let damage = ini
        .get(weapon_raw_name, "Damage")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(35.0)
        .max(1.0);
    let rof = ini
        .get(weapon_raw_name, "ROF")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(30.0);
    let cooldown_secs = if rof > 10.0 {
        (rof / 15.0).clamp(0.25, 10.0)
    } else {
        rof.clamp(0.25, 10.0)
    };
    let raw_range = ini
        .get(weapon_raw_name, "Range")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(5.0);
    let range = (raw_range * 2.5).clamp(3.0, 40.0);
    let projectile_speed = ini
        .get(weapon_raw_name, "Speed")
        .and_then(|v| v.parse::<f32>().ok())
        .map(|s| (s * 0.5).clamp(15.0, 80.0))
        .unwrap_or(35.0);

    let mut armor_multipliers = BTreeMap::new();
    if let Some(warhead) = ini.get(weapon_raw_name, "Warhead") {
        if let Some(verses) = ini.get(warhead, "Verses") {
            let pcts: Vec<f32> = verses
                .split(',')
                .filter_map(|p| p.trim().trim_end_matches('%').parse::<f32>().ok())
                .map(|v| (v / 100.0).clamp(0.05, 3.0))
                .collect();
            if pcts.len() >= 5 {
                armor_multipliers.insert(ArmorType::None, pcts[0]);
                armor_multipliers.insert(ArmorType::Light, pcts[1]);
                armor_multipliers.insert(ArmorType::Medium, pcts[2]);
                armor_multipliers.insert(ArmorType::Heavy, pcts[3]);
                armor_multipliers.insert(ArmorType::Structure, pcts[4]);
            }
        }
    }

    let burst = ini
        .get(weapon_raw_name, "Burst")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let is_electric = parse_bool(ini.get(weapon_raw_name, "IsElectricBolt"));
    let is_laser = parse_bool(ini.get(weapon_raw_name, "IsLaser"));
    let is_rad = parse_bool(ini.get(weapon_raw_name, "IsRadBeam"));
    let w_lower = weapon_raw_name.to_ascii_lowercase();
    let projectile_style = if is_electric || w_lower.contains("tesla") || w_lower.contains("bolt") {
        rts_sdk::schema::ProjectileStyle::Tesla
    } else if is_laser || w_lower.contains("prism") || w_lower.contains("laser") {
        rts_sdk::schema::ProjectileStyle::Laser
    } else if is_rad || w_lower.contains("rad") {
        rts_sdk::schema::ProjectileStyle::Radiation
    } else if w_lower.contains("mind") || w_lower.contains("psychic") {
        rts_sdk::schema::ProjectileStyle::MindControl
    } else if w_lower.contains("missile") || w_lower.contains("sam") || w_lower.contains("flak") {
        rts_sdk::schema::ProjectileStyle::Missile
    } else {
        rts_sdk::schema::ProjectileStyle::Bullet
    };

    WeaponDef {
        id,
        name: weapon_raw_name.to_string(),
        damage,
        range,
        cooldown_secs,
        projectile_speed,
        armor_multipliers,
        targets_ground: true,
        targets_air: w_lower.contains("flak")
            || w_lower.contains("sam")
            || w_lower.contains("aa")
            || w_lower.contains("redeye")
            || w_lower.contains("patriot"),
        splash_radius: if damage >= 85.0 { 2.2 } else { 0.0 },
        burst,
        projectile_style,
        on_hit_zone: (is_rad || w_lower.contains("rad"))
            .then(rts_sdk::schema::HazardZoneSpec::default),
        on_hit_status: w_lower.contains("assaultbolt").then(|| rts_sdk::schema::StatusEffectDef {
            id: "power_charge".to_string(),
            duration_secs: 6.0,
            power_charge_boost: true,
            ..Default::default()
        }),
        mind_control: w_lower.contains("mind") || w_lower.contains("psychic"),
        warp_erase: w_lower.contains("chrono") || w_lower.contains("warp"),
        ..Default::default()
    }
}

