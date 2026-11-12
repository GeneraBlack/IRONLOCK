use super::{check_existing_asset_binary, is_known_vfx_id, ValidationReport};
use crate::loader::LoadedMod;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_visual_sockets_and_pbr(
    loaded: &LoadedMod,
    visual: &crate::schema::VisualDef,
    owner_ctx: &str,
    require_model_files: bool,
    checked_binary_assets: &mut BTreeSet<String>,
    report: &mut ValidationReport,
) {
    let mut seen_names = BTreeSet::new();
    for sock in &visual.sockets {
        if sock.name.trim().is_empty() {
            report.push_error(
                "INVALID_SOCKET",
                format!("{owner_ctx} defines a visual socket with an empty name"),
            );
        } else if !seen_names.insert(sock.name.as_str()) {
            report.push_error(
                "DUPLICATE_SOCKET",
                format!(
                    "{owner_ctx} defines duplicate visual socket name '{}'",
                    sock.name
                ),
            );
        }
        if sock.offset.iter().any(|c| !c.is_finite()) {
            report.push_error(
                "INVALID_SOCKET",
                format!(
                    "{owner_ctx} socket '{}' has non-finite offset {:?}",
                    sock.name, sock.offset
                ),
            );
        }
    }

    // Phase 28 PBR material & team-mask checks
    if let Some(m) = visual.metallic {
        if !m.is_finite() || !(0.0..=1.0).contains(&m) {
            report.push_error(
                "INVALID_PBR_MATERIAL",
                format!("{owner_ctx} visual.metallic {m} must be in 0.0..=1.0"),
            );
        }
    }
    if let Some(r) = visual.roughness {
        if !r.is_finite() || !(0.0..=1.0).contains(&r) {
            report.push_error(
                "INVALID_PBR_MATERIAL",
                format!("{owner_ctx} visual.roughness {r} must be in 0.0..=1.0"),
            );
        }
    }
    if let Some(eb) = visual.emissive_boost {
        if !eb.is_finite() || eb < 0.0 {
            report.push_error(
                "INVALID_PBR_MATERIAL",
                format!("{owner_ctx} visual.emissive_boost {eb} must be >= 0.0"),
            );
        }
    }
    if let Some(thr) = visual.team_mask_threshold {
        if !thr.is_finite() || !(0.0..=1.0).contains(&thr) {
            report.push_error(
                "INVALID_PBR_MATERIAL",
                format!("{owner_ctx} visual.team_mask_threshold {thr} must be in 0.0..=1.0"),
            );
        }
    }
    for (field_label, opt_map) in [
        ("normal_map_path", &visual.normal_map_path),
        ("orm_map_path", &visual.orm_map_path),
    ] {
        if let Some(rel_path) = opt_map.as_deref().filter(|s| !s.is_empty()) {
            let full = loaded.resolve_asset_path(rel_path);
            if !loaded.asset_exists(rel_path) {
                let msg = format!(
                    "{owner_ctx} visual.{field_label} '{rel_path}' does not exist at {}",
                    full.display()
                );
                if require_model_files {
                    report.push_error("MISSING_TEXTURE", msg);
                } else {
                    report.push_warning("MISSING_TEXTURE", msg);
                }
            } else {
                check_existing_asset_binary(
                    loaded,
                    rel_path,
                    &format!("{owner_ctx} visual.{field_label}"),
                    checked_binary_assets,
                    report,
                );
            }
        }
    }
}

pub(super) fn validate_entity_resource_maps(
    costs: &BTreeMap<String, u32>,
    trickle: &BTreeMap<String, u32>,
    owner_ctx: &str,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) {
    for res_id in costs.keys() {
        if !loaded.resources.contains_key(res_id) {
            report.push_error(
                "UNKNOWN_RESOURCE_TYPE",
                format!("{owner_ctx} resource_costs references unknown resource '{res_id}'"),
            );
        }
    }
    for res_id in trickle.keys() {
        if !loaded.resources.contains_key(res_id) {
            report.push_error(
                "UNKNOWN_RESOURCE_TYPE",
                format!(
                    "{owner_ctx} resource_trickle_per_sec references unknown resource '{res_id}'"
                ),
            );
        }
    }
}

pub(super) fn validate_entity_traits(
    builder: Option<&crate::schema::BuilderTraitSpec>,
    gatherer: Option<&crate::schema::GathererTraitSpec>,
    squad: Option<&crate::schema::SquadTraitSpec>,
    owner_ctx: &str,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) {
    if let Some(bt) = builder {
        if !bt.build_speed_multiplier.is_finite()
            || bt.build_speed_multiplier <= 0.0
            || !bt.build_range.is_finite()
            || bt.build_range <= 0.0
        {
            report.push_error(
                "INVALID_BUILDER_TRAIT",
                format!(
                    "{owner_ctx} builder_trait must have positive finite build_speed_multiplier ({}) and build_range ({})",
                    bt.build_speed_multiplier, bt.build_range
                ),
            );
        }
        for b_id in &bt.buildable_buildings {
            if !loaded.buildings.contains_key(b_id) {
                report.push_error(
                    "UNKNOWN_BUILDABLE_BUILDING",
                    format!(
                        "{owner_ctx} builder_trait references unknown buildable building '{b_id}'"
                    ),
                );
            }
        }
    }
    if let Some(gt) = gatherer {
        if gt.carry_capacity == 0
            || !gt.gather_rate_per_sec.is_finite()
            || gt.gather_rate_per_sec <= 0.0
        {
            report.push_error(
                "INVALID_GATHERER_TRAIT",
                format!(
                    "{owner_ctx} gatherer_trait must have carry_capacity > 0 ({}) and positive finite gather_rate_per_sec ({})",
                    gt.carry_capacity, gt.gather_rate_per_sec
                ),
            );
        }
        for res_id in &gt.allowed_resources {
            if !loaded.resources.contains_key(res_id) {
                report.push_error(
                    "UNKNOWN_RESOURCE_TYPE",
                    format!(
                        "{owner_ctx} gatherer_trait allowed_resources references unknown resource '{res_id}'"
                    ),
                );
            }
        }
        for b_id in &gt.dropoff_buildings {
            if !loaded.buildings.contains_key(b_id) {
                report.push_error(
                    "UNKNOWN_DROPOFF_BUILDING",
                    format!(
                        "{owner_ctx} gatherer_trait dropoff_buildings references unknown building '{b_id}'"
                    ),
                );
            }
        }
    }
    if let Some(st) = squad {
        let opt_non_neg_finite = |opt: Option<f32>| opt.is_none_or(|v| v.is_finite() && v >= 0.0);
        if st.member_count == 0
            || !st.member_spacing.is_finite()
            || st.member_spacing <= 0.0
            || !st.reinforce_cost_ratio.is_finite()
            || st.reinforce_cost_ratio < 0.0
            || !st.directional_cover_multiplier.is_finite()
            || st.directional_cover_multiplier <= 0.0
            || !st.max_suppression.is_finite()
            || st.max_suppression <= 0.0
            || !opt_non_neg_finite(st.light_cover_damage_mult)
            || !opt_non_neg_finite(st.heavy_cover_damage_mult)
            || !opt_non_neg_finite(st.pin_suppression_threshold)
        {
            report.push_error(
                "INVALID_SQUAD_TRAIT",
                format!("{owner_ctx} squad_trait has invalid parameters: {st:?}"),
            );
        }
        for up_w in &st.available_weapon_upgrades {
            if !loaded.weapons.contains_key(up_w) && !loaded.upgrades.contains_key(up_w) {
                report.push_error(
                    "UNKNOWN_WEAPON",
                    format!(
                        "{owner_ctx} squad_trait available_weapon_upgrades references unknown weapon or upgrade '{up_w}'"
                    ),
                );
            }
        }
    }
}

pub(super) fn validate_hero_and_energy_traits(
    energy_pool: Option<&crate::schema::EnergyPoolSpec>,
    hero: Option<&crate::schema::HeroTraitSpec>,
    unit_abilities: &[crate::schema::AbilityDef],
    owner_ctx: &str,
    report: &mut ValidationReport,
) {
    if let Some(ep) = energy_pool {
        if !ep.max_energy.is_finite()
            || ep.max_energy <= 0.0
            || !ep.initial_energy.is_finite()
            || ep.initial_energy < 0.0
            || !ep.regen_per_sec.is_finite()
            || ep.regen_per_sec < 0.0
        {
            report.push_error(
                "INVALID_ENERGY_POOL",
                format!("{owner_ctx} energy_pool has invalid values: {ep:?}"),
            );
        }
    }
    if let Some(ht) = hero {
        if ht.max_level == 0
            || !ht.xp_share_radius.is_finite()
            || ht.xp_share_radius < 0.0
            || !ht.hp_per_level.is_finite()
            || !ht.damage_mult_per_level.is_finite()
            || !ht.energy_per_level.is_finite()
            || !ht.revive_cost_mult.is_finite()
            || ht.revive_cost_mult < 0.0
            || !ht.revive_time_per_level_secs.is_finite()
            || ht.revive_time_per_level_secs < 0.0
            || ht.xp_curve.iter().any(|x| !x.is_finite() || *x <= 0.0)
        {
            report.push_error(
                "INVALID_HERO_TRAIT",
                format!("{owner_ctx} hero_trait has invalid parameters: {ht:?}"),
            );
        }
        if !unit_abilities.is_empty() {
            let defined_ab_ids: BTreeSet<&str> =
                unit_abilities.iter().map(|a| a.id.as_str()).collect();
            for learn_ab in &ht.learnable_abilities {
                if !defined_ab_ids.contains(learn_ab.as_str()) {
                    report.push_error(
                        "UNKNOWN_HERO_ABILITY",
                        format!(
                            "{owner_ctx} hero_trait learnable_abilities references unknown ability '{learn_ab}'"
                        ),
                    );
                }
            }
        }
    }
}

pub(crate) fn validate_status_effect_refs(
    se: &crate::schema::StatusEffectDef,
    owner_ctx: &str,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) {
    if let Some(spawn_u) = &se.on_kill_spawn_unit {
        if !spawn_u.is_empty() && !loaded.units.contains_key(spawn_u) {
            report.push_error(
                "UNKNOWN_SPAWN_UNIT",
                format!(
                    "{owner_ctx} status '{}' references unknown on_kill_spawn_unit '{spawn_u}'",
                    se.id
                ),
            );
        }
    }
}

pub(crate) fn validate_effect_payload(
    payload: &crate::schema::EffectPayload,
    owner_ctx: &str,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) {
    match payload {
        crate::schema::EffectPayload::SpawnUnits {
            unit_id: Some(uid), ..
        } if !uid.is_empty() && !loaded.units.contains_key(uid) => {
            report.push_error(
                "UNKNOWN_SPAWN_UNIT",
                format!("{owner_ctx} SpawnUnits references unknown unit '{uid}'"),
            );
        }
        crate::schema::EffectPayload::MutateOrganicInfantry {
            into_unit_id: Some(uid),
            ..
        } if !uid.is_empty() && !loaded.units.contains_key(uid) => {
            report.push_error(
                "UNKNOWN_SPAWN_UNIT",
                format!("{owner_ctx} MutateOrganicInfantry references unknown unit '{uid}'"),
            );
        }
        crate::schema::EffectPayload::ApplyAreaStatus { status, .. } => {
            validate_status_effect_refs(status, owner_ctx, loaded, report);
        }
        _ => {}
    }
}

pub(crate) fn validate_ability(
    ab: &crate::schema::AbilityDef,
    owner_ctx: &str,
    loaded: &LoadedMod,
    report: &mut ValidationReport,
) {
    if !ab.energy_cost.is_finite() || ab.energy_cost < 0.0 || ab.max_rank == 0 {
        report.push_error(
            "INVALID_ABILITY",
            format!(
                "{owner_ctx} has invalid energy_cost ({}) or max_rank ({})",
                ab.energy_cost, ab.max_rank
            ),
        );
    }
    for vfx_id in [&ab.cast_vfx_id, &ab.impact_vfx_id].into_iter().flatten() {
        if !vfx_id.is_empty() && !is_known_vfx_id(loaded, vfx_id) {
            report.push_error(
                "UNKNOWN_VFX_EFFECT",
                format!("{owner_ctx} references unknown VFX effect '{vfx_id}'"),
            );
        }
    }
    if let Some(hook_fn) = &ab.script_hook {
        if !hook_fn.is_empty() && !loaded.script_host.has_hook(hook_fn) {
            report.push_error(
                "UNKNOWN_SCRIPT_HOOK",
                format!("{owner_ctx} references undefined script function '{hook_fn}'"),
            );
        }
    }
    if let Some(se) = &ab.applies_status {
        validate_status_effect_refs(se, owner_ctx, loaded, report);
    }
    for payload in &ab.payloads {
        validate_effect_payload(payload, owner_ctx, loaded, report);
    }
}
