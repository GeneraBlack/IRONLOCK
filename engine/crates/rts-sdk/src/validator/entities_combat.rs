use super::traits_visuals::{
    validate_ability, validate_effect_payload, validate_entity_resource_maps,
    validate_entity_traits, validate_hero_and_energy_traits, validate_visual_sockets_and_pbr,
};
use super::{check_existing_asset_binary, is_known_vfx_id, ValidationReport};
use crate::loader::LoadedMod;
use std::collections::BTreeSet;

pub(super) fn validate_entities_and_combat(
    loaded: &LoadedMod,
    require_model_files: bool,
    defined_eras: &BTreeSet<&str>,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    // Validate buildings
    for (b_id, b) in &loaded.buildings {
        let owner_ctx = format!("Building '{b_id}'");
        if !loaded.factions.contains_key(&b.faction) && !b.is_available_to_faction("") {
            report.push_error(
                "UNKNOWN_FACTION",
                format!(
                    "Building '{b_id}' references unknown faction '{}'",
                    b.faction
                ),
            );
        }
        if let Some(w_id) = &b.primary_weapon {
            if !loaded.weapons.contains_key(w_id) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!("Building '{b_id}' references unknown weapon '{w_id}'"),
                );
            }
        }
        for prereq in &b.prerequisites {
            if !loaded.buildings.contains_key(prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Building '{b_id}' requires unknown building '{prereq}'"),
                );
            }
        }
        for up_prereq in &b.prerequisite_upgrades {
            if !loaded.upgrades.contains_key(up_prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Building '{b_id}' requires unknown upgrade '{up_prereq}'"),
                );
            }
        }
        if let Some(era_id) = b.required_era.as_deref().filter(|s| !s.is_empty()) {
            if !defined_eras.contains(era_id) {
                report.push_error(
                    "UNKNOWN_ERA",
                    format!("Building '{b_id}' requires unknown era '{era_id}'"),
                );
            }
        }
        if let Some(sbp) = b.streaming_build_power {
            if !sbp.is_finite() || sbp < 0.0 {
                report.push_error(
                    "INVALID_STREAMING_BUILD_POWER",
                    format!("Building '{b_id}' has invalid streaming_build_power {sbp}"),
                );
            }
        }
        for vfx_id in [&b.ambient_vfx_id, &b.death_vfx_id].into_iter().flatten() {
            if !vfx_id.is_empty() && !is_known_vfx_id(loaded, vfx_id) {
                report.push_error(
                    "UNKNOWN_VFX_EFFECT",
                    format!("Building '{b_id}' references unknown VFX effect '{vfx_id}'"),
                );
            }
        }
        if let Some(spawn_unit) = &b.spawns_unit_on_complete {
            if !loaded.units.contains_key(spawn_unit) {
                report.push_error(
                    "UNKNOWN_SPAWN_UNIT",
                    format!("Building '{b_id}' spawns unknown unit '{spawn_unit}' on completion"),
                );
            }
        }
        for sp in &b.support_powers {
            if let Some(uid) = &sp.unit_id {
                if !uid.is_empty() && !loaded.units.contains_key(uid) {
                    report.push_error(
                        "UNKNOWN_SUPPORT_POWER_UNIT",
                        format!(
                            "Building '{b_id}' support power '{}' references unknown unit '{uid}'",
                            sp.id
                        ),
                    );
                }
            }
            for payload in &sp.payloads {
                validate_effect_payload(
                    payload,
                    &format!("Building '{b_id}' support power '{}'", sp.id),
                    loaded,
                    report,
                );
            }
        }
        for ab in &b.abilities {
            validate_ability(
                ab,
                &format!("Building '{b_id}' ability '{}'", ab.id),
                loaded,
                report,
            );
        }
        for unlocked_uid in &b.unlocks_units {
            if !loaded.units.contains_key(unlocked_uid) {
                report.push_error(
                    "UNKNOWN_UNLOCK_UNIT",
                    format!(
                        "Building '{b_id}' unlocks_units references unknown unit '{unlocked_uid}'"
                    ),
                );
            }
        }
        for stolen_uid in &b.stolen_tech_units {
            if !loaded.units.contains_key(stolen_uid) {
                report.push_error(
                    "UNKNOWN_STOLEN_TECH_UNIT",
                    format!(
                        "Building '{b_id}' stolen_tech_units references unknown unit '{stolen_uid}'"
                    ),
                );
            }
        }
        if let Some(trans_unit) = &b.transforms_into_unit {
            if !loaded.units.contains_key(trans_unit) {
                report.push_error(
                    "UNKNOWN_TRANSFORM_UNIT",
                    format!("Building '{b_id}' transforms into unknown unit '{trans_unit}'"),
                );
            }
        }
        if let Some(up_b) = &b.upgrades_to {
            if !loaded.buildings.contains_key(up_b) {
                report.push_error(
                    "UNKNOWN_UPGRADE_BUILDING",
                    format!("Building '{b_id}' upgrades to unknown building '{up_b}'"),
                );
            }
        }
        for opt_id in &b.upgrade_options {
            if !loaded.buildings.contains_key(opt_id) && !loaded.upgrades.contains_key(opt_id) {
                report.push_error(
                    "UNKNOWN_UPGRADE_BUILDING",
                    format!(
                        "Building '{b_id}' upgrade_options references unknown building or upgrade '{opt_id}'"
                    ),
                );
            }
        }
        if let Some(surv_u) = b.death_payload_spec().survivor_unit.as_ref() {
            if !loaded.units.contains_key(surv_u) {
                report.push_error(
                    "UNKNOWN_SURVIVOR_UNIT",
                    format!("Building '{b_id}' references unknown survivor unit '{surv_u}'"),
                );
            }
        }
        for (event, hook_fn) in &b.script_hooks {
            if !loaded.script_host.has_hook(hook_fn) {
                report.push_error(
                    "UNKNOWN_SCRIPT_HOOK",
                    format!(
                        "Building '{b_id}' hook '{event}' references undefined script function '{hook_fn}'"
                    ),
                );
            }
        }
        check_model_and_icon(
            loaded,
            &b.visual,
            &owner_ctx,
            require_model_files,
            checked_binary_assets,
            report,
        );
        validate_visual_sockets_and_pbr(
            loaded,
            &b.visual,
            &owner_ctx,
            require_model_files,
            checked_binary_assets,
            report,
        );
        validate_entity_resource_maps(
            &b.resource_costs,
            &b.resource_trickle_per_sec,
            &owner_ctx,
            loaded,
            report,
        );
        validate_entity_traits(
            b.builder_trait.as_ref(),
            b.gatherer_trait.as_ref(),
            b.squad_trait.as_ref(),
            &owner_ctx,
            loaded,
            report,
        );
        validate_hero_and_energy_traits(
            b.energy_pool.as_ref(),
            None,
            &b.abilities,
            &owner_ctx,
            report,
        );
    }

    // Detect prerequisite cycles in buildings
    for b_id in loaded.buildings.keys() {
        let mut visited = BTreeSet::new();
        let mut stack = vec![b_id.as_str()];
        while let Some(curr) = stack.pop() {
            if !visited.insert(curr) {
                continue;
            }
            if let Some(def) = loaded.buildings.get(curr) {
                for p in &def.prerequisites {
                    if p == b_id {
                        report.push_error(
                            "TECH_TREE_CYCLE",
                            format!("Cyclic building prerequisite detected involving '{b_id}'"),
                        );
                        break;
                    }
                    stack.push(p.as_str());
                }
            }
        }
    }

    // Validate units
    for (u_id, u) in &loaded.units {
        let owner_ctx = format!("Unit '{u_id}'");
        if !loaded.factions.contains_key(&u.faction) && !u.is_available_to_faction("") {
            report.push_error(
                "UNKNOWN_FACTION",
                format!("Unit '{u_id}' references unknown faction '{}'", u.faction),
            );
        }
        for w_id in [
            &u.primary_weapon,
            &u.secondary_weapon,
            &u.elite_weapon,
            &u.deploy_weapon,
        ]
        .into_iter()
        .flatten()
        {
            if !loaded.weapons.contains_key(w_id) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!("Unit '{u_id}' references unknown weapon '{w_id}'"),
                );
            }
        }
        for (pass_uid, pass_wid) in &u.passenger_weapon_by_unit {
            if !loaded.units.contains_key(pass_uid) {
                report.push_error(
                    "UNKNOWN_PASSENGER_UNIT",
                    format!(
                        "Unit '{u_id}' passenger_weapon_by_unit references unknown unit '{pass_uid}'"
                    ),
                );
            }
            if !loaded.weapons.contains_key(pass_wid) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!(
                        "Unit '{u_id}' passenger_weapon_by_unit['{pass_uid}'] references unknown weapon '{pass_wid}'"
                    ),
                );
            }
        }
        for (role, pass_wid) in &u.passenger_weapon_by_role {
            if !loaded.weapons.contains_key(pass_wid) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!(
                        "Unit '{u_id}' passenger_weapon_by_role['{role:?}'] references unknown weapon '{pass_wid}'"
                    ),
                );
            }
        }
        if let Some(trans_b) = &u.transforms_into_building {
            if !loaded.buildings.contains_key(trans_b) {
                report.push_error(
                    "UNKNOWN_TRANSFORM_BUILDING",
                    format!("Unit '{u_id}' transforms into unknown building '{trans_b}'"),
                );
            }
        }
        if let Some(surv_u) = u.death_payload_spec().survivor_unit.as_ref() {
            if !loaded.units.contains_key(surv_u) {
                report.push_error(
                    "UNKNOWN_SURVIVOR_UNIT",
                    format!("Unit '{u_id}' references unknown survivor unit '{surv_u}'"),
                );
            }
        }
        if let Some(drone_u) = &u.spawns_drone_unit {
            if !loaded.units.contains_key(drone_u) {
                report.push_error(
                    "UNKNOWN_DRONE_UNIT",
                    format!("Unit '{u_id}' references unknown drone unit '{drone_u}'"),
                );
            }
        }
        if let Some(req_pow_b) = &u.requires_powered_building {
            if !req_pow_b.is_empty() && !loaded.buildings.contains_key(req_pow_b) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!(
                        "Unit '{u_id}' requires_powered_building references unknown building '{req_pow_b}'"
                    ),
                );
            }
        }
        for prereq in &u.prerequisites {
            if !loaded.buildings.contains_key(prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Unit '{u_id}' requires unknown building '{prereq}'"),
                );
            }
        }
        for up_prereq in &u.prerequisite_upgrades {
            if !loaded.upgrades.contains_key(up_prereq) {
                report.push_error(
                    "UNKNOWN_PREREQUISITE",
                    format!("Unit '{u_id}' requires unknown upgrade '{up_prereq}'"),
                );
            }
        }
        if let Some(era_id) = u.required_era.as_deref().filter(|s| !s.is_empty()) {
            if !defined_eras.contains(era_id) {
                report.push_error(
                    "UNKNOWN_ERA",
                    format!("Unit '{u_id}' requires unknown era '{era_id}'"),
                );
            }
        }
        if let Some(sbp) = u.streaming_build_power {
            if !sbp.is_finite() || sbp < 0.0 {
                report.push_error(
                    "INVALID_STREAMING_BUILD_POWER",
                    format!("Unit '{u_id}' has invalid streaming_build_power {sbp}"),
                );
            }
        }
        for vfx_id in [&u.ambient_vfx_id, &u.death_vfx_id].into_iter().flatten() {
            if !vfx_id.is_empty() && !is_known_vfx_id(loaded, vfx_id) {
                report.push_error(
                    "UNKNOWN_VFX_EFFECT",
                    format!("Unit '{u_id}' references unknown VFX effect '{vfx_id}'"),
                );
            }
        }
        for ab in &u.abilities {
            validate_ability(
                ab,
                &format!("Unit '{u_id}' ability '{}'", ab.id),
                loaded,
                report,
            );
        }
        for (event, hook_fn) in &u.script_hooks {
            if !loaded.script_host.has_hook(hook_fn) {
                report.push_error(
                    "UNKNOWN_SCRIPT_HOOK",
                    format!(
                        "Unit '{u_id}' hook '{event}' references undefined script function '{hook_fn}'"
                    ),
                );
            }
        }
        check_model_and_icon(
            loaded,
            &u.visual,
            &owner_ctx,
            require_model_files,
            checked_binary_assets,
            report,
        );
        validate_visual_sockets_and_pbr(
            loaded,
            &u.visual,
            &owner_ctx,
            require_model_files,
            checked_binary_assets,
            report,
        );
        validate_entity_resource_maps(
            &u.resource_costs,
            &u.resource_trickle_per_sec,
            &owner_ctx,
            loaded,
            report,
        );
        validate_entity_traits(
            u.builder_trait.as_ref(),
            u.gatherer_trait.as_ref(),
            u.squad_trait.as_ref(),
            &owner_ctx,
            loaded,
            report,
        );
        validate_hero_and_energy_traits(
            u.energy_pool.as_ref(),
            u.hero_trait.as_ref(),
            &u.abilities,
            &owner_ctx,
            report,
        );
        let loco = &u.locomotor;
        if !loco.turn_rate_deg_per_sec.is_finite()
            || loco.turn_rate_deg_per_sec < 0.0
            || !loco.turret_slew_deg_per_sec.is_finite()
            || loco.turret_slew_deg_per_sec < 0.0
            || !loco.accel.is_finite()
            || loco.accel < 0.0
            || !loco.decel.is_finite()
            || loco.decel < 0.0
            || !loco.attack_run_radius.is_finite()
            || loco.attack_run_radius < 0.0
        {
            report.push_error(
                "INVALID_LOCOMOTOR",
                format!("Unit '{u_id}' has negative or non-finite locomotor rates: {loco:?}"),
            );
        }
    }

    validate_combat_semantic_lints(loaded, report);
}

fn check_model_and_icon(
    loaded: &LoadedMod,
    visual: &crate::schema::VisualDef,
    owner_ctx: &str,
    require_model_files: bool,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    let model_path = loaded.resolve_asset_path(&visual.model);
    if !loaded.asset_exists(&visual.model) {
        let msg = format!(
            "{owner_ctx} 3D model '{}' does not exist at {}",
            visual.model,
            model_path.display()
        );
        if require_model_files {
            report.push_error("MISSING_MODEL", msg);
        } else {
            report.push_warning("MISSING_MODEL", msg);
        }
    } else {
        check_existing_asset_binary(
            loaded,
            &visual.model,
            &format!("{owner_ctx} 3D model"),
            checked_binary_assets,
            report,
        );
    }
    if let Some(icon_rel) = &visual.icon {
        if !icon_rel.is_empty() {
            let icon_path = loaded.resolve_asset_path(icon_rel);
            if !loaded.asset_exists(icon_rel) {
                let msg = format!(
                    "{owner_ctx} cameo icon '{icon_rel}' does not exist at {}",
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
                    &format!("{owner_ctx} cameo icon"),
                    checked_binary_assets,
                    report,
                );
            }
        }
    }
}

fn validate_combat_semantic_lints(loaded: &LoadedMod, report: &mut ValidationReport) {
    for (w_id, w) in &loaded.weapons {
        if w.damage > 0.0
            && !w.armor_multipliers.is_empty()
            && w.armor_multipliers.values().all(|&m| m <= 0.0)
        {
            report.push_error(
                "ZERO_EFFECTIVE_DAMAGE",
                format!(
                    "Weapon '{w_id}' has damage {} > 0.0, but all armor_multipliers are <= 0.0",
                    w.damage
                ),
            );
        }
    }

    let all_stolen_tech_units: BTreeSet<&str> = loaded
        .buildings
        .values()
        .flat_map(|b| b.stolen_tech_units.iter().map(|s| s.as_str()))
        .collect();

    for (b_id, b) in &loaded.buildings {
        if b.is_available_to_faction("") {
            continue;
        }
        for prereq in &b.prerequisites {
            if let Some(prereq_b) = loaded.buildings.get(prereq) {
                if !prereq_b.is_available_to_faction("")
                    && !prereq_b.is_available_to_faction(&b.faction)
                {
                    report.push_error(
                        "UNREACHABLE_PREREQUISITE",
                        format!(
                            "Building '{b_id}' (faction '{}') requires unreachable cross-faction prerequisite '{prereq}' (faction '{}')",
                            b.faction, prereq_b.faction
                        ),
                    );
                }
            }
        }
    }
    for (u_id, u) in &loaded.units {
        if u.is_available_to_faction("") || all_stolen_tech_units.contains(u_id.as_str()) {
            continue;
        }
        for prereq in &u.prerequisites {
            if let Some(prereq_b) = loaded.buildings.get(prereq) {
                if !prereq_b.is_available_to_faction("")
                    && !prereq_b.is_available_to_faction(&u.faction)
                {
                    report.push_error(
                        "UNREACHABLE_PREREQUISITE",
                        format!(
                            "Unit '{u_id}' (faction '{}') requires unreachable cross-faction prerequisite '{prereq}' (faction '{}')",
                            u.faction, prereq_b.faction
                        ),
                    );
                }
            }
        }
    }

    let mut referenced_weapons: BTreeSet<&str> = BTreeSet::new();
    for u in loaded.units.values() {
        for w in [
            &u.primary_weapon,
            &u.secondary_weapon,
            &u.elite_weapon,
            &u.deploy_weapon,
        ]
        .into_iter()
        .flatten()
        {
            referenced_weapons.insert(w.as_str());
        }
        for w in u.passenger_weapon_by_unit.values() {
            referenced_weapons.insert(w.as_str());
        }
        for w in u.passenger_weapon_by_role.values() {
            referenced_weapons.insert(w.as_str());
        }
        if let Some(st) = &u.squad_trait {
            for w in &st.available_weapon_upgrades {
                referenced_weapons.insert(w.as_str());
            }
        }
    }
    for b in loaded.buildings.values() {
        if let Some(w) = &b.primary_weapon {
            referenced_weapons.insert(w.as_str());
        }
        if let Some(st) = &b.squad_trait {
            for w in &st.available_weapon_upgrades {
                referenced_weapons.insert(w.as_str());
            }
        }
    }
    for (w_id, w) in &loaded.weapons {
        if !referenced_weapons.contains(w_id.as_str()) {
            report.push_warning(
                "ORPHANED_WEAPON",
                format!("Weapon '{w_id}' is not referenced by any unit or building"),
            );
        }
        for vfx_id in [&w.muzzle_vfx_id, &w.projectile_vfx_id, &w.impact_vfx_id]
            .into_iter()
            .flatten()
        {
            if !vfx_id.is_empty() && !is_known_vfx_id(loaded, vfx_id) {
                report.push_error(
                    "UNKNOWN_VFX_EFFECT",
                    format!("Weapon '{w_id}' references unknown VFX effect '{vfx_id}'"),
                );
            }
        }
        for se in w.iter_on_hit_statuses() {
            if se.duration_secs <= 0.0 {
                report.push_warning(
                    "ORPHANED_STATUS_EFFECT",
                    format!(
                        "Status effect '{}' on weapon '{w_id}' has zero or negative duration ({})",
                        se.id, se.duration_secs
                    ),
                );
            }
            if let Some(spawn_u) = &se.on_kill_spawn_unit {
                if !spawn_u.is_empty() && !loaded.units.contains_key(spawn_u) {
                    report.push_error(
                        "UNKNOWN_SPAWN_UNIT",
                        format!(
                            "Status effect '{}' on weapon '{w_id}' spawns unknown unit '{spawn_u}'",
                            se.id
                        ),
                    );
                }
            }
        }
        let bal = &w.ballistics;
        if !bal.gravity.is_finite()
            || bal.gravity < 0.0
            || !bal.scatter_radius.is_finite()
            || bal.scatter_radius < 0.0
            || !bal.moving_scatter_multiplier.is_finite()
            || bal.moving_scatter_multiplier < 0.0
            || !bal.lead_factor.is_finite()
            || bal.lead_factor < 0.0
            || !bal.suppression_amount.is_finite()
            || bal.suppression_amount < 0.0
            || !bal.turn_rate_deg_per_sec.is_finite()
            || bal.turn_rate_deg_per_sec < 0.0
            || !bal.cone_half_angle_deg.is_finite()
            || bal.cone_half_angle_deg < 0.0
        {
            report.push_error(
                "INVALID_BALLISTICS",
                format!(
                    "Weapon '{w_id}' has negative or non-finite ballistics parameters: {bal:?}"
                ),
            );
        }
    }
}
