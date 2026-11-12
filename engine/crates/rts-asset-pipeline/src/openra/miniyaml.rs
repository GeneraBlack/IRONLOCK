use crate::westwood::normalize_id;
use rts_sdk::schema::{ArmorType, ProjectileStyle, WeaponDef};
use std::collections::{BTreeMap, BTreeSet};

/// A node in an OpenRA MiniYAML document tree.
#[derive(Debug, Clone, Default)]
pub struct MiniYamlNode {
    pub key: String,
    pub value: String,
    pub children: Vec<MiniYamlNode>,
}

impl MiniYamlNode {
    pub fn child(&self, key_prefix: &str) -> Option<&MiniYamlNode> {
        self.children.iter().find(|c| {
            c.key.eq_ignore_ascii_case(key_prefix)
                || c.key
                    .split('@')
                    .next()
                    .map(|base| base.eq_ignore_ascii_case(key_prefix))
                    .unwrap_or(false)
        })
    }

    pub fn children_matching(&self, key_prefix: &str) -> Vec<&MiniYamlNode> {
        self.children
            .iter()
            .filter(|c| {
                c.key.eq_ignore_ascii_case(key_prefix)
                    || c.key
                        .split('@')
                        .next()
                        .map(|base| base.eq_ignore_ascii_case(key_prefix))
                        .unwrap_or(false)
            })
            .collect()
    }

    pub fn child_value(&self, key_prefix: &str) -> Option<&str> {
        self.child(key_prefix)
            .map(|c| c.value.as_str())
            .filter(|v| !v.is_empty())
    }

    pub fn nested_value(&self, trait_name: &str, prop_name: &str) -> Option<&str> {
        for t in self.children_matching(trait_name) {
            if let Some(val) = t.child_value(prop_name) {
                return Some(val);
            }
        }
        None
    }
}

pub(super) fn measure_indent(raw_line: &str) -> usize {
    let mut indent = 0;
    for ch in raw_line.chars() {
        if ch == '\t' {
            indent += 4;
        } else if ch == ' ' {
            indent += 1;
        } else {
            break;
        }
    }
    indent
}

/// Parses an OpenRA MiniYAML string into top-level `MiniYamlNode` entries.
pub fn parse_miniyaml(input: &str) -> Vec<MiniYamlNode> {
    let mut roots: Vec<MiniYamlNode> = Vec::new();
    let mut stack: Vec<(usize, MiniYamlNode)> = Vec::new();

    for raw_line in input.lines() {
        let without_comment = raw_line.split('#').next().unwrap_or("");
        if without_comment.trim().is_empty() {
            continue;
        }
        let indent = measure_indent(without_comment);
        let trimmed = without_comment.trim();
        let (key, value) = if let Some((k, v)) = trimmed.split_once(':') {
            (k.trim().to_string(), v.trim().to_string())
        } else {
            (trimmed.to_string(), String::new())
        };

        let node = MiniYamlNode {
            key,
            value,
            children: Vec::new(),
        };

        while stack.last().map(|(ind, _)| *ind >= indent).unwrap_or(false) {
            let (_, completed) = stack.pop().unwrap();
            if let Some((_, parent)) = stack.last_mut() {
                parent.children.push(completed);
            } else {
                roots.push(completed);
            }
        }

        stack.push((indent, node));
    }

    while let Some((_, completed)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(completed);
        } else {
            roots.push(completed);
        }
    }

    roots
}

/// Parses OpenRA cell-subcell distances (`"5c0"`, `"4c512"`, `"6"`) into world units.
pub fn parse_openra_range(raw: &str) -> Option<f32> {
    let s = raw.trim().to_ascii_lowercase();
    if let Some((cells_str, sub_str)) = s.split_once('c') {
        let cells: f32 = cells_str.trim().parse().ok()?;
        let sub: f32 = sub_str.trim().parse().unwrap_or(0.0);
        Some(((cells + sub / 1024.0) * 2.5).clamp(2.0, 45.0))
    } else {
        let val: f32 = s.parse().ok()?;
        Some((val * 2.5).clamp(2.0, 45.0))
    }
}

pub(super) fn map_openra_armor(raw: Option<&str>, is_building: bool) -> ArmorType {
    let Some(a) = raw else {
        return if is_building {
            ArmorType::Structure
        } else {
            ArmorType::Light
        };
    };
    match a.trim().to_ascii_lowercase().as_str() {
        "none" => ArmorType::None,
        "light" | "flak" => ArmorType::Light,
        "medium" => ArmorType::Medium,
        "heavy" => ArmorType::Heavy,
        "wood" | "concrete" | "steel" | "structure" | "building" => ArmorType::Structure,
        _ => {
            if is_building {
                ArmorType::Structure
            } else {
                ArmorType::Light
            }
        }
    }
}

pub(super) fn detect_openra_major_factions(actor: &MiniYamlNode) -> Vec<String> {
    let mut combined = String::new();
    if let Some(p) = actor.nested_value("Buildable", "Prerequisites") {
        combined.push_str(p);
        combined.push(' ');
    }
    if let Some(f) = actor.nested_value("Buildable", "Factions") {
        combined.push_str(f);
        combined.push(' ');
    }
    if let Some(o) = actor.nested_value("Buildable", "Owner") {
        combined.push_str(o);
        combined.push(' ');
    }
    for prov in actor.children_matching("ProvidesPrerequisite") {
        if let Some(pr) = prov.child_value("Prerequisite") {
            combined.push_str(pr);
            combined.push(' ');
        }
    }
    if let Some(v) = actor.nested_value("Voiced", "VoiceSet") {
        combined.push_str(v);
    }
    let lower = combined.to_ascii_lowercase();
    let key_lower = actor.key.to_ascii_lowercase();

    let mut majors = Vec::new();
    let mut push_unique = |f: &str| {
        if !majors.iter().any(|existing| existing == f) {
            majors.push(f.to_string());
        }
    };

    if lower.contains("yuri")
        || lower.contains("epsilon")
        || lower.contains("~ya")
        || lower.contains("yapowr")
        || lower.contains("yabrc")
        || lower.contains("yaweap")
        || lower.contains("yatech")
        || key_lower.starts_with("ya")
    {
        push_unique("yuri");
    }
    if lower.contains("gdi") {
        push_unique("gdi");
    }
    if lower.contains("nod") {
        push_unique("nod");
    }
    if lower.contains("atreides") {
        push_unique("atreides");
    }
    if lower.contains("harkonnen") {
        push_unique("harkonnen");
    }
    if lower.contains("ordos") {
        push_unique("ordos");
    }
    if lower.contains("scrin") || lower.contains("cabal") {
        push_unique("scrin");
    }
    if lower.contains("allies")
        || lower.contains("allied")
        || lower.contains("america")
        || lower.contains("germany")
        || lower.contains("england")
        || lower.contains("france")
        || lower.contains("korea")
        || lower.contains("~ga")
        || lower.contains("gapile")
        || lower.contains("gaweap")
        || lower.contains("garefn")
        || lower.contains("gaairc")
        || lower.contains("gatech")
        || lower.contains("gayard")
    {
        push_unique("allies");
    }
    if lower.contains("soviet")
        || lower.contains("russia")
        || lower.contains("ukraine")
        || lower.contains("iraq")
        || lower.contains("cuba")
        || lower.contains("libya")
        || lower.contains("~na")
        || lower.contains("nahand")
        || lower.contains("naweap")
        || lower.contains("naradr")
        || lower.contains("natech")
        || lower.contains("nayard")
        || lower.contains("narefn")
        || lower.contains("napowr")
        || lower.contains("nacnst")
        || key_lower.starts_with("na")
        || key_lower == "tsla"
        || key_lower == "bnk"
    {
        push_unique("soviets");
    }

    if majors.is_empty() {
        majors.push("allies".to_string());
    }
    majors
}

pub(super) fn infer_openra_faction(actor: &MiniYamlNode) -> String {
    detect_openra_major_factions(actor)
        .into_iter()
        .next()
        .unwrap_or_else(|| "allies".to_string())
}

pub(super) fn clean_openra_prerequisites(
    raw: Option<&str>,
    faction: &str,
    known_buildings: &BTreeSet<String>,
    provided_by_faction: &BTreeMap<(String, String), String>,
    provided_any: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(s) = raw else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for tok in s.split(',').map(str::trim) {
        if tok.is_empty() || tok.starts_with('!') || tok.starts_with("~!") {
            continue;
        }
        let stripped = tok.trim_start_matches('~').trim();
        if stripped.is_empty() || stripped.eq_ignore_ascii_case("disabled") {
            continue;
        }
        let norm = normalize_id(stripped);
        let resolved = if known_buildings.contains(&norm) {
            Some(norm)
        } else if let Some(b_id) = provided_by_faction.get(&(faction.to_string(), norm.clone())) {
            Some(b_id.clone())
        } else {
            provided_any.get(&norm).cloned()
        };
        if let Some(id) = resolved {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

pub(super) fn merge_inherited_traits(
    actor: &MiniYamlNode,
    templates: &BTreeMap<String, MiniYamlNode>,
    depth: usize,
) -> MiniYamlNode {
    if depth > 4 {
        return actor.clone();
    }
    let mut merged = MiniYamlNode {
        key: actor.key.clone(),
        value: actor.value.clone(),
        children: Vec::new(),
    };

    for inh in actor.children_matching("Inherits") {
        let parent_key = inh.value.trim();
        if parent_key.eq_ignore_ascii_case(&actor.key) {
            continue;
        }
        if let Some(tpl) = templates.get(parent_key) {
            let resolved_tpl = merge_inherited_traits(tpl, templates, depth + 1);
            for tpl_child in &resolved_tpl.children {
                merged.children.push(tpl_child.clone());
            }
        }
    }
    for child in &actor.children {
        if let Some(existing) = merged
            .children
            .iter_mut()
            .find(|c| c.key.eq_ignore_ascii_case(&child.key))
        {
            if !child.value.is_empty() {
                existing.value = child.value.clone();
            }
            for sub in &child.children {
                if let Some(ex_sub) = existing
                    .children
                    .iter_mut()
                    .find(|s| s.key.eq_ignore_ascii_case(&sub.key))
                {
                    ex_sub.value = sub.value.clone();
                } else {
                    existing.children.push(sub.clone());
                }
            }
        } else {
            merged.children.push(child.clone());
        }
    }
    merged
}

pub(super) fn extract_openra_sub_factions(actor: &MiniYamlNode) -> Vec<String> {
    const COUNTRIES: [&str; 9] = [
        "america", "germany", "england", "france", "korea", "russia", "iraq", "libya", "cuba",
    ];
    let mut out: Vec<String> = detect_openra_major_factions(actor)
        .into_iter()
        .skip(1)
        .collect();
    let mut raw = String::new();
    if let Some(p) = actor.nested_value("Buildable", "Prerequisites") {
        raw.push_str(p);
        raw.push(',');
    }
    if let Some(f) = actor.nested_value("Buildable", "Factions") {
        raw.push_str(f);
    }
    for tok in raw.split(',').map(str::trim) {
        if tok.starts_with('!') || tok.starts_with("~!") {
            continue;
        }
        let lower = tok.to_ascii_lowercase();
        for c in COUNTRIES {
            if lower.contains(c) && !out.iter().any(|s| s == c) {
                out.push(c.to_string());
            }
        }
    }
    out
}

pub(super) fn parse_openra_weapon_node(node: &MiniYamlNode) -> WeaponDef {
    let id = normalize_id(&node.key);
    let reload_delay = node
        .child_value("ReloadDelay")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(30.0);
    let cooldown_secs = (reload_delay / 20.0).clamp(0.25, 10.0);
    let range = node
        .child_value("Range")
        .and_then(parse_openra_range)
        .unwrap_or(14.0);

    let projectile_speed = node
        .child("Projectile")
        .and_then(|p| p.child_value("Speed"))
        .and_then(parse_openra_range)
        .map(|s| (s * 4.0).clamp(18.0, 80.0))
        .unwrap_or(35.0);

    let burst = node
        .child_value("Burst")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);

    let valid_targets = node
        .child_value("ValidTargets")
        .unwrap_or("Ground, Water")
        .to_ascii_lowercase();
    let targets_air = valid_targets.contains("air");
    let targets_ground = valid_targets.contains("ground")
        || valid_targets.contains("water")
        || valid_targets.contains("teslaboost")
        || !targets_air;

    let proj_type = node
        .child("Projectile")
        .map(|p| p.value.to_ascii_lowercase())
        .unwrap_or_default();
    let id_lower = id.to_ascii_lowercase();

    let mut damage = 40.0f32;
    let mut armor_multipliers = BTreeMap::new();
    let mut splash_radius = 0.0f32;
    let mut leaves_radiation = proj_type.contains("rad") || id_lower.contains("rad");
    let mut mind_control =
        id_lower.contains("mind") || id_lower.contains("psychic") || id_lower.contains("psi");
    let mut warp_erase = id_lower.contains("chrono") || id_lower.contains("warp");
    let mut tesla_charge_boost = valid_targets.contains("teslaboost");

    for wh in node.children_matching("Warhead") {
        let wh_type = wh.value.to_ascii_lowercase();
        if wh_type.contains("createtintedcells") {
            leaves_radiation = true;
        }
        if wh_type.contains("warpdamage") {
            warp_erase = true;
        }
        if wh_type.contains("grantexternalcondition") {
            if let Some(cond) = wh.child_value("Condition") {
                if cond.eq_ignore_ascii_case("charged") {
                    tesla_charge_boost = true;
                } else if cond.to_ascii_lowercase().contains("psi") {
                    mind_control = true;
                }
            }
        }
        if let Some(spread_raw) = wh.child_value("Spread") {
            if let Ok(subcells) = spread_raw.trim().parse::<f32>() {
                splash_radius = (subcells / 96.0).clamp(0.8, 6.0);
            } else if let Some(r) = parse_openra_range(spread_raw) {
                splash_radius = r.clamp(0.8, 6.0);
            }
        }
        if let Some(dmg_raw) = wh.child_value("Damage") {
            if let Ok(parsed) = dmg_raw.parse::<f32>() {
                damage = if parsed >= 1000.0 {
                    (parsed / 100.0).max(5.0)
                } else {
                    parsed.max(5.0)
                };
            }
        }
        if let Some(versus) = wh.child("Versus") {
            for v_child in &versus.children {
                if let Ok(pct) = v_child.value.trim_end_matches('%').parse::<f32>() {
                    let armor = map_openra_armor(Some(&v_child.key), false);
                    armor_multipliers.insert(armor, (pct / 100.0).clamp(0.05, 3.0));
                }
            }
        }
    }

    let projectile_style = if mind_control {
        ProjectileStyle::MindControl
    } else if leaves_radiation {
        ProjectileStyle::Radiation
    } else if proj_type.contains("electric")
        || id_lower.contains("bolt")
        || id_lower.contains("tesla")
    {
        ProjectileStyle::Tesla
    } else if proj_type.contains("laser")
        || id_lower.contains("prism")
        || id_lower.contains("laser")
    {
        ProjectileStyle::Laser
    } else if proj_type.contains("areabeam")
        || id_lower.contains("sonic")
        || id_lower.contains("wave")
    {
        ProjectileStyle::Sonic
    } else if proj_type.contains("missile") || targets_air {
        ProjectileStyle::Missile
    } else {
        ProjectileStyle::Bullet
    };

    WeaponDef {
        id,
        name: node.key.clone(),
        damage,
        range,
        cooldown_secs,
        projectile_speed,
        armor_multipliers,
        targets_ground,
        targets_air,
        splash_radius,
        burst,
        projectile_style,
        on_hit_zone: leaves_radiation.then(rts_sdk::schema::HazardZoneSpec::default),
        on_hit_status: tesla_charge_boost.then(|| rts_sdk::schema::StatusEffectDef {
            id: "power_charge".to_string(),
            duration_secs: 6.0,
            power_charge_boost: true,
            ..Default::default()
        }),
        mind_control,
        warp_erase,
        ..Default::default()
    }
}
