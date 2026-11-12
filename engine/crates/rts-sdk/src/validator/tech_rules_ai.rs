use super::traits_visuals::validate_ability;
use super::{check_existing_asset_binary, ValidationReport};
use crate::loader::LoadedMod;
use crate::schema::{AiBtActionKind, BlackboardConditionKind, BtNodeKind};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_rules_upgrades_items_trees_and_ai(
    loaded: &LoadedMod,
    require_model_files: bool,
    defined_eras: &BTreeSet<&str>,
    defined_support_powers: &BTreeSet<&str>,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    let rules = &loaded.manifest.rules;
    for (f_id, u_id) in &rules.free_unit_by_faction {
        if !loaded.factions.contains_key(f_id) {
            report.push_error(
                "UNKNOWN_RULE_FACTION",
                format!("manifest.rules.free_unit_by_faction references unknown faction '{f_id}'"),
            );
        }
        if !u_id.is_empty() && !loaded.units.contains_key(u_id) {
            report.push_error(
                "UNKNOWN_RULE_UNIT",
                format!(
                    "manifest.rules.free_unit_by_faction['{f_id}'] references unknown unit '{u_id}'"
                ),
            );
        }
    }
    for (f_id, u_id) in &rules.default_infantry_by_faction {
        if !loaded.factions.contains_key(f_id) {
            report.push_error(
                "UNKNOWN_RULE_FACTION",
                format!(
                    "manifest.rules.default_infantry_by_faction references unknown faction '{f_id}'"
                ),
            );
        }
        if !u_id.is_empty() && !loaded.units.contains_key(u_id) {
            report.push_error(
                "UNKNOWN_RULE_UNIT",
                format!(
                    "manifest.rules.default_infantry_by_faction['{f_id}'] references unknown unit '{u_id}'"
                ),
            );
        }
    }
    for (pair_key, u_id) in &rules.stolen_tech_commando_by_pair {
        if let Some((f1, f2)) = pair_key.split_once(':') {
            for f_id in [f1, f2] {
                if f_id != "*" && !loaded.factions.contains_key(f_id) {
                    report.push_error(
                        "UNKNOWN_RULE_FACTION",
                        format!(
                            "manifest.rules.stolen_tech_commando_by_pair['{pair_key}'] references unknown faction '{f_id}'"
                        ),
                    );
                }
            }
        } else {
            report.push_error(
                "INVALID_RULE_PAIR",
                format!(
                    "manifest.rules.stolen_tech_commando_by_pair key '{pair_key}' must have 'faction_a:faction_b' format"
                ),
            );
        }
        if !u_id.is_empty() && !loaded.units.contains_key(u_id) {
            report.push_error(
                "UNKNOWN_RULE_UNIT",
                format!(
                    "manifest.rules.stolen_tech_commando_by_pair['{pair_key}'] references unknown unit '{u_id}'"
                ),
            );
        }
    }
    if !rules.default_liberated_worker_unit.is_empty()
        && !loaded
            .units
            .contains_key(&rules.default_liberated_worker_unit)
    {
        report.push_error(
            "UNKNOWN_RULE_UNIT",
            format!(
                "manifest.rules.default_liberated_worker_unit references unknown unit '{}'",
                rules.default_liberated_worker_unit
            ),
        );
    }
    if !rules.default_mutant_unit.is_empty()
        && !loaded.units.contains_key(&rules.default_mutant_unit)
    {
        report.push_error(
            "UNKNOWN_RULE_UNIT",
            format!(
                "manifest.rules.default_mutant_unit references unknown unit '{}'",
                rules.default_mutant_unit
            ),
        );
    }

    // Validate Phase 25 eras, damage_matrix, and gamemode rules
    let mut seen_era_ids = BTreeSet::new();
    for era in &rules.eras {
        if era.id.trim().is_empty() {
            report.push_error("INVALID_ERA_DEF", "manifest.rules.eras contains empty id");
        } else if !seen_era_ids.insert(era.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!("manifest.rules.eras defines duplicate era id '{}'", era.id),
            );
        }
        for b_id in &era.prerequisite_buildings {
            if !loaded.buildings.contains_key(b_id) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Era '{}' requires unknown building '{b_id}'", era.id),
                );
            }
        }
        for up_id in &era.prerequisite_upgrades {
            if !loaded.upgrades.contains_key(up_id) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Era '{}' requires unknown upgrade '{up_id}'", era.id),
                );
            }
        }
        for res_id in era.resource_costs.keys() {
            if !loaded.resources.contains_key(res_id) {
                report.push_error(
                    "UNKNOWN_RESOURCE_TYPE",
                    format!("Era '{}' references unknown resource '{res_id}'", era.id),
                );
            }
        }
    }

    for (wh_tag, row) in &rules.damage_matrix.warhead_vs_armor {
        for (ar_tag, &mult) in row {
            if !mult.is_finite() || mult < 0.0 {
                report.push_error(
                    "INVALID_DAMAGE_MATRIX",
                    format!(
                        "damage_matrix['{wh_tag}']['{ar_tag}'] multiplier {mult} must be finite and >= 0.0"
                    ),
                );
            }
        }
    }

    for (w_idx, wave) in rules.gamemode.survival_waves.iter().enumerate() {
        if wave.unit_id.trim().is_empty() || !loaded.units.contains_key(&wave.unit_id) {
            report.push_error(
                "UNKNOWN_RULE_UNIT",
                format!(
                    "manifest.rules.gamemode.survival_waves[{w_idx}] references unknown unit '{}'",
                    wave.unit_id
                ),
            );
        }
    }

    // Validate upgrades & doctrines
    for (up_id, up) in &loaded.upgrades {
        if !loaded.factions.contains_key(&up.faction) && !up.is_available_to_faction("") {
            report.push_error(
                "UNKNOWN_FACTION",
                format!(
                    "Upgrade '{up_id}' references unknown faction '{}'",
                    up.faction
                ),
            );
        }
        for prereq in &up.prerequisites {
            if !loaded.buildings.contains_key(prereq) && !loaded.upgrades.contains_key(prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Upgrade '{up_id}' requires unknown building or upgrade '{prereq}'"),
                );
            } else if !up.is_available_to_faction("") {
                if let Some(prereq_b) = loaded.buildings.get(prereq) {
                    if !prereq_b.is_available_to_faction("")
                        && !prereq_b.is_available_to_faction(&up.faction)
                    {
                        report.push_error(
                            "UNREACHABLE_PREREQUISITE",
                            format!(
                                "Upgrade '{up_id}' (faction '{}') requires unreachable cross-faction building '{prereq}' (faction '{}')",
                                up.faction, prereq_b.faction
                            ),
                        );
                    }
                }
            }
        }
        for up_prereq in &up.prerequisite_upgrades {
            if !loaded.upgrades.contains_key(up_prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Upgrade '{up_id}' requires unknown upgrade '{up_prereq}'"),
                );
            }
        }
        for (era_label, opt_era) in [
            ("required_era", &up.required_era),
            ("advances_to_era", &up.advances_to_era),
        ] {
            if let Some(era_id) = opt_era.as_deref().filter(|s| !s.is_empty()) {
                if !defined_eras.contains(era_id) {
                    report.push_error(
                        "UNKNOWN_ERA",
                        format!("Upgrade '{up_id}' {era_label} references unknown era '{era_id}'"),
                    );
                }
            }
        }
        for def_id in &up.target_def_ids {
            if !loaded.units.contains_key(def_id) && !loaded.buildings.contains_key(def_id) {
                report.push_error(
                    "UNKNOWN_TARGET_ENTITY",
                    format!(
                        "Upgrade '{up_id}' target_def_ids references unknown unit or building '{def_id}'"
                    ),
                );
            }
        }
        for res_id in up.resource_costs.keys() {
            if !loaded.resources.contains_key(res_id) {
                report.push_error(
                    "UNKNOWN_RESOURCE_TYPE",
                    format!("Upgrade '{up_id}' references unknown resource '{res_id}'"),
                );
            }
        }
    }
    for up_id in loaded.upgrades.keys() {
        let mut visited = BTreeSet::new();
        let mut stack = vec![up_id.as_str()];
        while let Some(curr) = stack.pop() {
            if !visited.insert(curr) {
                continue;
            }
            if let Some(def) = loaded.upgrades.get(curr) {
                for p in def
                    .prerequisites
                    .iter()
                    .chain(def.prerequisite_upgrades.iter())
                {
                    if p == up_id {
                        report.push_error(
                            "TECH_TREE_CYCLE",
                            format!("Cyclic upgrade prerequisite detected involving '{up_id}'"),
                        );
                        break;
                    }
                    if loaded.upgrades.contains_key(p) {
                        stack.push(p.as_str());
                    }
                }
            }
        }
    }

    // Validate ItemCatalog (`loaded.items`)
    for (item_id, item) in &loaded.items {
        if item.id.trim().is_empty() || item.max_charges == 0 {
            report.push_error(
                "INVALID_ITEM_DEF",
                format!("Item '{item_id}' has empty id or zero max_charges"),
            );
        }
        for (field, opt_rel) in [("icon", &item.icon), ("visual_model", &item.visual_model)] {
            if let Some(rel) = opt_rel.as_deref().filter(|s| !s.is_empty()) {
                let full = loaded.resolve_asset_path(rel);
                if !loaded.asset_exists(rel) {
                    let msg = format!(
                        "Item '{item_id}' {field} '{rel}' does not exist at {}",
                        full.display()
                    );
                    if require_model_files {
                        report.push_error("MISSING_ICON", msg);
                    } else {
                        report.push_warning("MISSING_ICON", msg);
                    }
                } else {
                    check_existing_asset_binary(
                        loaded,
                        rel,
                        &format!("Item '{item_id}' {field}"),
                        checked_binary_assets,
                        report,
                    );
                }
            }
        }
        if let Some(ab) = &item.active_ability {
            validate_ability(
                ab,
                &format!("Item '{item_id}' active_ability '{}'", ab.id),
                loaded,
                report,
            );
        }
    }

    validate_commander_trees(
        loaded,
        require_model_files,
        defined_support_powers,
        checked_binary_assets,
        report,
    );
    validate_ai_profiles(loaded, defined_eras, defined_support_powers, report);
}

fn validate_commander_trees(
    loaded: &LoadedMod,
    require_model_files: bool,
    defined_support_powers: &BTreeSet<&str>,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    for (tree_id, tree) in &loaded.commander_trees {
        if !tree.is_available_to_faction("") && !loaded.factions.contains_key(&tree.faction) {
            report.push_error(
                "UNKNOWN_FACTION",
                format!(
                    "CommanderPowerTree '{tree_id}' references unknown faction '{}'",
                    tree.faction
                ),
            );
        }
        let mut node_map = BTreeMap::new();
        for node in &tree.nodes {
            if node.id.trim().is_empty() {
                report.push_error(
                    "INVALID_COMMANDER_NODE",
                    format!("CommanderPowerTree '{tree_id}' defines a node with empty id"),
                );
                continue;
            }
            if node_map.insert(node.id.as_str(), node).is_some() {
                report.push_error(
                    "DUPLICATE_ID",
                    format!(
                        "CommanderPowerTree '{tree_id}' defines duplicate node id '{}'",
                        node.id
                    ),
                );
            }
        }
        for node in &tree.nodes {
            for prereq in &node.prerequisites {
                if !node_map.contains_key(prereq.as_str())
                    && !loaded.buildings.contains_key(prereq)
                    && !loaded.upgrades.contains_key(prereq)
                {
                    report.push_error(
                        "UNKNOWN_PREREQUISITE",
                        format!(
                            "CommanderPowerTree '{tree_id}' node '{}' requires unknown prerequisite '{prereq}'",
                            node.id
                        ),
                    );
                }
            }
            if let Some(sp_id) = node
                .grants_support_power
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                if !defined_support_powers.contains(sp_id) {
                    report.push_error(
                        "UNKNOWN_SUPPORT_POWER",
                        format!(
                            "CommanderPowerTree '{tree_id}' node '{}' grants unknown support power '{sp_id}'",
                            node.id
                        ),
                    );
                }
            }
            if let Some(up_id) = node.grants_upgrade.as_deref().filter(|s| !s.is_empty()) {
                if !loaded.upgrades.contains_key(up_id) {
                    report.push_error(
                        "UNKNOWN_UPGRADE",
                        format!(
                            "CommanderPowerTree '{tree_id}' node '{}' grants unknown upgrade '{up_id}'",
                            node.id
                        ),
                    );
                }
            }
            if let Some(def_id) = node.unlocks_def_id.as_deref().filter(|s| !s.is_empty()) {
                if !loaded.units.contains_key(def_id) && !loaded.buildings.contains_key(def_id) {
                    report.push_error(
                        "UNKNOWN_UNLOCK_UNIT",
                        format!(
                            "CommanderPowerTree '{tree_id}' node '{}' unlocks unknown entity '{def_id}'",
                            node.id
                        ),
                    );
                }
            }
            if let Some(icon_rel) = node.icon.as_deref().filter(|s| !s.is_empty()) {
                let icon_path = loaded.resolve_asset_path(icon_rel);
                if !loaded.asset_exists(icon_rel) {
                    let msg = format!(
                        "CommanderPowerTree '{tree_id}' node '{}' icon '{icon_rel}' does not exist at {}",
                        node.id,
                        icon_path.display()
                    );
                    if require_model_files {
                        report.push_error("MISSING_ICON", msg);
                    } else {
                        report.push_warning("MISSING_ICON", msg);
                    }
                } else {
                    check_existing_asset_binary(
                        loaded,
                        icon_rel,
                        &format!("CommanderPowerTree '{tree_id}' node '{}' icon", node.id),
                        checked_binary_assets,
                        report,
                    );
                }
            }
        }
        for start_id in node_map.keys() {
            let mut visited = BTreeSet::new();
            let mut stack = vec![*start_id];
            while let Some(curr) = stack.pop() {
                if !visited.insert(curr) {
                    continue;
                }
                if let Some(n_def) = node_map.get(curr) {
                    for p in &n_def.prerequisites {
                        if p == start_id {
                            report.push_error(
                                "TECH_TREE_CYCLE",
                                format!(
                                    "Cyclic commander tree node prerequisite detected in '{tree_id}' involving '{start_id}'"
                                ),
                            );
                            break;
                        }
                        if node_map.contains_key(p.as_str()) {
                            stack.push(p.as_str());
                        }
                    }
                }
            }
        }
    }
}

fn validate_ai_profiles(
    loaded: &LoadedMod,
    defined_eras: &BTreeSet<&str>,
    defined_support_powers: &BTreeSet<&str>,
    report: &mut ValidationReport,
) {
    for (p_id, prof) in &loaded.ai_profiles {
        if !loaded.factions.contains_key(&prof.faction)
            && !matches!(prof.faction.as_str(), "any" | "shared" | "*")
        {
            report.push_error(
                "UNKNOWN_AI_PROFILE_FACTION",
                format!(
                    "AI profile '{p_id}' references unknown faction '{}'",
                    prof.faction
                ),
            );
        }
        for b_id in &prof.build_order {
            if !loaded.buildings.contains_key(b_id) {
                report.push_error(
                    "UNKNOWN_AI_BUILD_ORDER_BUILDING",
                    format!("AI profile '{p_id}' build_order references unknown building '{b_id}'"),
                );
            }
        }
        for u_id in prof.unit_weights.keys() {
            if !loaded.units.contains_key(u_id) {
                report.push_error(
                    "UNKNOWN_AI_UNIT_WEIGHT",
                    format!("AI profile '{p_id}' unit_weights references unknown unit '{u_id}'"),
                );
            }
        }
        for sw_id in &prof.preferred_superweapons {
            if !defined_support_powers.contains(sw_id.as_str()) {
                report.push_error(
                    "UNKNOWN_AI_SUPERWEAPON",
                    format!(
                        "AI profile '{p_id}' preferred_superweapons references unknown support power '{sw_id}'"
                    ),
                );
            }
        }
        if let Some(hook_fn) = &prof.on_ai_tick {
            if !loaded.script_host.has_hook(hook_fn) {
                report.push_error(
                    "UNKNOWN_SCRIPT_HOOK",
                    format!(
                        "AI profile '{p_id}' on_ai_tick references undefined script function '{hook_fn}'"
                    ),
                );
            }
        }
        if let Some(bt) = &prof.behavior_tree {
            validate_behavior_tree(bt, p_id, loaded, defined_eras, report);
        }
    }
}

fn validate_behavior_tree(
    bt: &crate::schema::BehaviorTreeDef,
    profile_id: &str,
    loaded: &LoadedMod,
    defined_eras: &BTreeSet<&str>,
    report: &mut ValidationReport,
) {
    let mut node_ids = BTreeSet::new();
    for node in &bt.nodes {
        if node.id.trim().is_empty() {
            report.push_error(
                "INVALID_BEHAVIOR_TREE",
                format!("AI profile '{profile_id}' behavior_tree contains a node with empty id"),
            );
        } else if !node_ids.insert(node.id.as_str()) {
            report.push_error(
                "DUPLICATE_ID",
                format!(
                    "AI profile '{profile_id}' behavior_tree defines duplicate node id '{}'",
                    node.id
                ),
            );
        }
    }
    if bt.root_node.trim().is_empty() || !node_ids.contains(bt.root_node.as_str()) {
        report.push_error(
            "INVALID_BEHAVIOR_TREE",
            format!(
                "AI profile '{profile_id}' behavior_tree root_node '{}' does not exist",
                bt.root_node
            ),
        );
    }
    if bt.has_cycle() {
        report.push_error(
            "BEHAVIOR_TREE_CYCLE",
            format!(
                "AI profile '{profile_id}' behavior_tree '{}' contains a cycle",
                bt.id
            ),
        );
    }
    for node in &bt.nodes {
        for child_id in node.node_kind.child_ids() {
            if !node_ids.contains(child_id) {
                report.push_error(
                    "UNKNOWN_BT_CHILD",
                    format!(
                        "AI profile '{profile_id}' behavior_tree node '{}' references unknown child '{child_id}'",
                        node.id
                    ),
                );
            }
        }
        match &node.node_kind {
            BtNodeKind::Cooldown { cooldown_secs, .. }
                if !cooldown_secs.is_finite() || *cooldown_secs < 0.0 =>
            {
                report.push_error(
                    "INVALID_BEHAVIOR_TREE",
                    format!(
                        "AI profile '{profile_id}' behavior_tree node '{}' has invalid cooldown {cooldown_secs}",
                        node.id
                    ),
                );
            }
            BtNodeKind::BlackboardCondition { condition } => match condition {
                BlackboardConditionKind::ResourceAbove { resource_id, .. }
                    if !loaded.resources.contains_key(resource_id) =>
                {
                    report.push_error(
                        "UNKNOWN_RESOURCE_TYPE",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' references unknown resource '{resource_id}'",
                            node.id
                        ),
                    );
                }
                BlackboardConditionKind::HasEra { era_id }
                    if !defined_eras.contains(era_id.as_str()) =>
                {
                    report.push_error(
                        "UNKNOWN_ERA",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' references unknown era '{era_id}'",
                            node.id
                        ),
                    );
                }
                BlackboardConditionKind::HasUpgrade { upgrade_id }
                    if !loaded.upgrades.contains_key(upgrade_id) =>
                {
                    report.push_error(
                        "UNKNOWN_UPGRADE",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' references unknown upgrade '{upgrade_id}'",
                            node.id
                        ),
                    );
                }
                BlackboardConditionKind::HasBuilding { building_id }
                    if !loaded.buildings.contains_key(building_id) =>
                {
                    report.push_error(
                        "UNKNOWN_AI_BUILD_ORDER_BUILDING",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' references unknown building '{building_id}'",
                            node.id
                        ),
                    );
                }
                _ => {}
            },
            BtNodeKind::AiAction { action } => match action {
                AiBtActionKind::TrainUnit {
                    unit_id: Some(uid), ..
                } if !uid.is_empty() && !loaded.units.contains_key(uid) => {
                    report.push_error(
                        "UNKNOWN_AI_UNIT_WEIGHT",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' TrainUnit references unknown unit '{uid}'",
                            node.id
                        ),
                    );
                }
                AiBtActionKind::ConstructBuilding { building_id }
                    if !loaded.buildings.contains_key(building_id) =>
                {
                    report.push_error(
                        "UNKNOWN_AI_BUILD_ORDER_BUILDING",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' ConstructBuilding references unknown building '{building_id}'",
                            node.id
                        ),
                    );
                }
                AiBtActionKind::ResearchUpgrade { upgrade_id }
                    if !loaded.upgrades.contains_key(upgrade_id) =>
                {
                    report.push_error(
                        "UNKNOWN_UPGRADE",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' ResearchUpgrade references unknown upgrade '{upgrade_id}'",
                            node.id
                        ),
                    );
                }
                AiBtActionKind::AdvanceEra {
                    era_id: Some(era_id),
                } if !era_id.is_empty() && !defined_eras.contains(era_id.as_str()) => {
                    report.push_error(
                        "UNKNOWN_ERA",
                        format!(
                            "AI profile '{profile_id}' BT node '{}' AdvanceEra references unknown era '{era_id}'",
                            node.id
                        ),
                    );
                }
                _ => {}
            },
            _ => {}
        }
    }
}
