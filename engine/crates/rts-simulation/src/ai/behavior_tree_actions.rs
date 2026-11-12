use crate::ai::behavior_tree::{is_combat_unit, is_hero_entity, BtEvalContext};
use crate::ai::{emit_ai_place_or_worker_build, AiCommanderState};
use crate::command::GameCommand;
use crate::det_math::det_distance_xz_f32;
use crate::world::EntityKind;
use rts_sdk::schema::{AbilityTargetMode, AiBtActionKind, BuildingRole, UnitRole};

pub(super) fn execute_bt_action(
    action: &AiBtActionKind,
    ctx: &BtEvalContext<'_>,
    cmdr: &AiCommanderState,
    commands: &mut Vec<GameCommand>,
) -> bool {
    let pid = ctx.ai_player_id;
    match action {
        AiBtActionKind::ExecuteBuildOrder => {
            let mut emitted = false;
            if let Some(prof) = ctx.profile {
                for b_id in &prof.build_order {
                    if ctx.owned_buildings.contains(b_id) {
                        continue;
                    }
                    if ctx.avail_buildings.iter().any(|ab| ab == b_id) {
                        if let Some(b_def) = ctx.loaded.buildings.get(b_id) {
                            let costs = b_def.effective_resource_costs();
                            if crate::world::economy::can_afford_costs(ctx.econ, &costs) {
                                emit_ai_place_or_worker_build(
                                    ctx.world,
                                    pid,
                                    b_def,
                                    None,
                                    ctx.has_conyard,
                                    ctx.loaded,
                                    commands,
                                );
                                emitted = true;
                            }
                        }
                        break;
                    }
                }
            }
            if !ctx.econ.is_supply_capped() {
                if let Some(u_def) = ctx
                    .avail_units
                    .iter()
                    .filter_map(|uid| ctx.loaded.units.get(uid))
                    .filter(|u| {
                        let existing_same = ctx
                            .world
                            .entities
                            .values()
                            .filter(|e| e.player_id == pid && e.health > 0.0 && e.def_id() == u.id)
                            .count();
                        if u.hero_trait.is_some() && existing_same >= 1 {
                            return false;
                        }
                        if (u.role == UnitRole::Harvester || u.builder_trait.is_some())
                            && existing_same >= 3
                        {
                            return false;
                        }
                        crate::world::economy::can_afford_costs(
                            ctx.econ,
                            &u.effective_resource_costs(),
                        )
                    })
                    .max_by_key(|u| {
                        let base_w = ctx
                            .profile
                            .and_then(|p| p.unit_weights.get(&u.id).copied())
                            .unwrap_or(50) as u64;
                        base_w * cmdr.counter_weight_percent(u, ctx.loaded)
                    })
                {
                    commands.push(GameCommand::ProduceUnit {
                        player_id: pid,
                        producer_entity_id: None,
                        unit_id: u_def.id.clone(),
                    });
                    emitted = true;
                }
            }
            emitted
        }
        AiBtActionKind::TrainUnit { unit_id, role_tag } => {
            if ctx.econ.is_supply_capped() {
                return false;
            }
            let chosen = if let Some(uid) = unit_id.as_deref().filter(|s| !s.is_empty()) {
                ctx.avail_units
                    .iter()
                    .any(|a| a == uid)
                    .then(|| ctx.loaded.units.get(uid))
                    .flatten()
            } else {
                ctx.avail_units
                    .iter()
                    .filter_map(|uid| ctx.loaded.units.get(uid))
                    .filter(|u| {
                        role_tag.as_deref().is_none_or(|rt| {
                            u.tags.iter().any(|t| t.eq_ignore_ascii_case(rt))
                                || format!("{:?}", u.role).eq_ignore_ascii_case(rt)
                        })
                    })
                    .max_by_key(|u| u.cost)
            };
            let Some(u_def) = chosen else {
                return false;
            };
            if !crate::world::economy::can_afford_costs(ctx.econ, &u_def.effective_resource_costs())
            {
                return false;
            }
            commands.push(GameCommand::ProduceUnit {
                player_id: pid,
                producer_entity_id: None,
                unit_id: u_def.id.clone(),
            });
            true
        }
        AiBtActionKind::ConstructBuilding { building_id } => {
            if !ctx.avail_buildings.iter().any(|b| b == building_id) {
                return false;
            }
            let Some(b_def) = ctx.loaded.buildings.get(building_id) else {
                return false;
            };
            if !crate::world::economy::can_afford_costs(ctx.econ, &b_def.effective_resource_costs())
            {
                return false;
            }
            let before = commands.len();
            emit_ai_place_or_worker_build(
                ctx.world,
                pid,
                b_def,
                None,
                ctx.has_conyard,
                ctx.loaded,
                commands,
            );
            commands.len() > before
        }
        AiBtActionKind::ResearchUpgrade { upgrade_id } => {
            if ctx.econ.researched_upgrades.contains(upgrade_id) {
                return false;
            }
            let Some(upg) = ctx.loaded.upgrades.get(upgrade_id) else {
                return false;
            };
            if !ctx.world.has_tech_and_era_prerequisites(
                pid,
                &upg.prerequisites,
                &upg.prerequisite_upgrades,
                upg.required_era.as_deref(),
                ctx.loaded,
            ) || !crate::world::economy::can_afford_costs(
                ctx.econ,
                &upg.effective_resource_costs(),
            ) {
                return false;
            }
            let researcher_id = ctx.world.entities.values().find_map(|e| {
                if e.player_id == pid
                    && ctx.world.is_building_online(e)
                    && matches!(
                        &e.kind,
                        EntityKind::Building { building_id }
                            if upg.prerequisites.is_empty() || upg.prerequisites.contains(building_id)
                    )
                    && ctx
                        .world
                        .trait_stores
                        .production
                        .get(&e.id)
                        .is_none_or(|pf| pf.local_research_queue.is_empty())
                {
                    Some(e.id)
                } else {
                    None
                }
            });
            commands.push(GameCommand::ResearchUpgradeAt {
                player_id: pid,
                building_id: researcher_id,
                upgrade_id: upgrade_id.clone(),
            });
            true
        }
        AiBtActionKind::AdvanceEra { era_id } => {
            let target_era_opt = era_id.clone().or_else(|| {
                let eras = &ctx.loaded.manifest.rules.eras;
                let curr_idx = ctx
                    .econ
                    .current_era
                    .as_deref()
                    .and_then(|cid| eras.iter().position(|e| e.id == cid))
                    .unwrap_or(0);
                eras.get(curr_idx + 1).map(|e| e.id.clone())
            });
            let Some(target_era) = target_era_opt else {
                return false;
            };
            if ctx.econ.current_era.as_deref() == Some(target_era.as_str()) {
                return false;
            }
            if let Some(upg) = ctx.loaded.upgrades.values().find(|u| {
                u.advances_to_era.as_deref() == Some(target_era.as_str())
                    && !ctx.econ.researched_upgrades.contains(&u.id)
                    && ctx.world.has_tech_and_era_prerequisites(
                        pid,
                        &u.prerequisites,
                        &u.prerequisite_upgrades,
                        u.required_era.as_deref(),
                        ctx.loaded,
                    )
                    && crate::world::economy::can_afford_costs(
                        ctx.econ,
                        &u.effective_resource_costs(),
                    )
            }) {
                commands.push(GameCommand::ResearchUpgradeAt {
                    player_id: pid,
                    building_id: None,
                    upgrade_id: upg.id.clone(),
                });
                return true;
            }
            if let Some(era_def) = ctx
                .loaded
                .manifest
                .rules
                .eras
                .iter()
                .find(|e| e.id == target_era)
            {
                if ctx.world.has_tech_and_era_prerequisites(
                    pid,
                    &era_def.prerequisite_buildings,
                    &era_def.prerequisite_upgrades,
                    None,
                    ctx.loaded,
                ) && crate::world::economy::can_afford_costs(ctx.econ, &era_def.resource_costs)
                {
                    commands.push(GameCommand::AdvanceEra {
                        player_id: pid,
                        era_id: target_era,
                        building_id: None,
                    });
                    return true;
                }
            }
            false
        }
        AiBtActionKind::ExpandOutpost => {
            if let Some(ref_def) = ctx
                .avail_buildings
                .iter()
                .filter_map(|bid| ctx.loaded.buildings.get(bid))
                .find(|b| b.role == BuildingRole::Refinery)
            {
                if crate::world::economy::can_afford_costs(
                    ctx.econ,
                    &ref_def.effective_resource_costs(),
                ) {
                    let before = commands.len();
                    emit_ai_place_or_worker_build(
                        ctx.world,
                        pid,
                        ref_def,
                        None,
                        ctx.has_conyard,
                        ctx.loaded,
                        commands,
                    );
                    return commands.len() > before;
                }
            }
            false
        }
        AiBtActionKind::LaunchAttackWave { min_units }
        | AiBtActionKind::LaunchAmphibiousWave { min_units } => {
            let attackers: Vec<u64> = ctx
                .world
                .entities
                .values()
                .filter(|e| e.player_id == pid && is_combat_unit(e, ctx.loaded))
                .map(|e| e.id)
                .collect();
            if attackers.len() < (*min_units).max(1) as usize {
                return false;
            }
            let target_pos = ctx
                .world
                .entities
                .values()
                .filter(|e| {
                    ctx.world.are_hostile(pid, e.player_id)
                        && e.health > 0.0
                        && e.transported_by.is_none()
                })
                .min_by_key(|e| {
                    let d = det_distance_xz_f32(e.position, ctx.base_world);
                    ((d * 10.0) as i64, e.id)
                })
                .map(|e| e.position);
            let Some(goal) = target_pos else {
                return false;
            };
            commands.push(GameCommand::AttackMove {
                player_id: pid,
                entity_ids: attackers,
                target_pos: goal,
            });
            true
        }
        AiBtActionKind::CastSuperweapon => {
            let avail = ctx
                .world
                .available_support_powers_for_player(pid, ctx.loaded);
            let Some((sp_def, _)) = avail.into_iter().find(|(_, cd)| *cd <= 0.0) else {
                return false;
            };
            let Some(enemy) = ctx
                .world
                .entities
                .values()
                .find(|e| ctx.world.are_hostile(pid, e.player_id) && e.health > 0.0)
            else {
                return false;
            };
            commands.push(GameCommand::CastSupportPower {
                player_id: pid,
                power_id: sp_def.id.clone(),
                source_pos: Some(ctx.staging_world),
                target_pos: enemy.position,
            });
            true
        }
        AiBtActionKind::RetreatWounded { health_ratio_below } => {
            let wounded: Vec<u64> = ctx
                .world
                .entities
                .values()
                .filter(|e| {
                    e.player_id == pid
                        && is_combat_unit(e, ctx.loaded)
                        && (e.health / e.max_health.max(1.0)) < *health_ratio_below
                        && det_distance_xz_f32(e.position, ctx.base_world) > 8.0
                })
                .map(|e| e.id)
                .collect();
            if wounded.is_empty() {
                return false;
            }
            commands.push(GameCommand::MoveUnits {
                player_id: pid,
                entity_ids: wounded,
                target_pos: ctx.base_world,
            });
            true
        }
        AiBtActionKind::DefendBase => {
            let Some(intruder) = ctx
                .world
                .entities
                .values()
                .filter(|e| {
                    ctx.world.are_hostile(pid, e.player_id)
                        && e.health > 0.0
                        && det_distance_xz_f32(e.position, ctx.base_world) <= 30.0
                })
                .min_by_key(|e| {
                    let d = det_distance_xz_f32(e.position, ctx.base_world);
                    ((d * 10.0) as i64, e.id)
                })
            else {
                return false;
            };
            let defenders: Vec<u64> = ctx
                .world
                .entities
                .values()
                .filter(|e| e.player_id == pid && is_combat_unit(e, ctx.loaded))
                .map(|e| e.id)
                .collect();
            if defenders.is_empty() {
                return false;
            }
            commands.push(GameCommand::AttackMove {
                player_id: pid,
                entity_ids: defenders,
                target_pos: intruder.position,
            });
            true
        }
        AiBtActionKind::CaptureControlPoint => {
            let target_zone = ctx
                .world
                .capture_zones
                .iter()
                .filter(|cz| {
                    cz.owner_player_id != Some(pid)
                        && cz
                            .owner_player_id
                            .is_none_or(|owner| ctx.world.are_hostile(pid, owner))
                })
                .min_by_key(|cz| {
                    let zw = ctx.world.grid.tile_to_world(cz.tile_pos);
                    (
                        (det_distance_xz_f32(zw, ctx.base_world) * 10.0) as i64,
                        &cz.id,
                    )
                });
            let Some(cz) = target_zone else {
                return false;
            };
            let squad: Vec<u64> = ctx
                .world
                .entities
                .values()
                .filter(|e| e.player_id == pid && is_combat_unit(e, ctx.loaded))
                .take(4)
                .map(|e| e.id)
                .collect();
            if squad.is_empty() {
                return false;
            }
            commands.push(GameCommand::AttackMove {
                player_id: pid,
                entity_ids: squad,
                target_pos: ctx.world.grid.tile_to_world(cz.tile_pos),
            });
            true
        }
        AiBtActionKind::UseHeroSkills => {
            let mut acted = false;
            for hero in ctx.world.entities.values().filter(|e| {
                e.player_id == pid && e.health > 0.0 && is_hero_entity(ctx.world, e, ctx.loaded)
            }) {
                if let Some(hs) = ctx.world.trait_stores.heroes.get(&hero.id) {
                    if hs.skill_points > 0 {
                        if let EntityKind::Unit { unit_id } = &hero.kind {
                            if let Some(h_spec) = ctx
                                .loaded
                                .units
                                .get(unit_id)
                                .and_then(|u| u.hero_trait.as_ref())
                            {
                                if let Some(ab_id) = h_spec.learnable_abilities.iter().find(|aid| {
                                    hs.learned_skills.get(*aid).copied().unwrap_or(0) < 4
                                }) {
                                    commands.push(GameCommand::LearnHeroSkill {
                                        player_id: pid,
                                        hero_id: hero.id,
                                        ability_id: ab_id.clone(),
                                    });
                                    acted = true;
                                }
                            }
                        }
                    }
                }
                if let EntityKind::Unit { unit_id } = &hero.kind {
                    if let Some(u_def) = ctx.loaded.units.get(unit_id) {
                        for (ab_idx, ab) in u_def.abilities.iter().enumerate() {
                            if hero.ability_cooldown_at(ab_idx) > 0.0
                                || (ab.energy_cost > 0.0 && hero.energy < ab.energy_cost)
                            {
                                continue;
                            }
                            let cast_r = ab.range.max(ab.radius).max(6.0);
                            if let Some(enemy) = ctx.world.entities.values().find(|other| {
                                ctx.world.are_hostile(pid, other.player_id)
                                    && other.health > 0.0
                                    && det_distance_xz_f32(hero.position, other.position) <= cast_r
                            }) {
                                if ab.target_mode == AbilityTargetMode::SelfOnly {
                                    commands.push(GameCommand::ActivateAbilityIndex {
                                        player_id: pid,
                                        entity_ids: vec![hero.id],
                                        ability_index: ab_idx,
                                    });
                                } else {
                                    commands.push(GameCommand::CastTargetedAbilityIndex {
                                        player_id: pid,
                                        caster_ids: vec![hero.id],
                                        ability_index: ab_idx,
                                        target_pos: enemy.position,
                                        target_entity_id: Some(enemy.id),
                                    });
                                }
                                acted = true;
                                break;
                            }
                        }
                    }
                }
            }
            acted
        }
    }
}
