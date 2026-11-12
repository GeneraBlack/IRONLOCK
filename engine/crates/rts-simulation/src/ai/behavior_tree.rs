use crate::ai::behavior_tree_actions::execute_bt_action;
use crate::ai::{AiCommanderState, AiDifficulty};
use crate::command::GameCommand;
use crate::det_math::{det_distance_xz_f32, SimScalar};
use crate::world::{EntityKind, PlayerEconomy, SimEntity, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    AiProfileDef, BehaviorTreeDef, BlackboardConditionKind, BtNodeKind, UnitRole,
};
use std::collections::BTreeSet;

/// Type alias for the deterministic per-commander AI Blackboard (`Task 243-Sim`).
pub type AiBlackboard = std::collections::BTreeMap<String, f32>;

pub(super) fn resolve_player_era_order(econ: &PlayerEconomy, loaded: &LoadedMod) -> u32 {
    let eras = &loaded.manifest.rules.eras;
    if let Some(curr) = econ.current_era.as_deref() {
        if let Some((idx, era_def)) = eras.iter().enumerate().find(|(_, e)| e.id == curr) {
            return if era_def.order > 0 {
                era_def.order
            } else {
                (idx as u32) + 1
            };
        }
        if let Some(num) = curr.rsplit('_').next().and_then(|s| s.parse::<u32>().ok()) {
            return num;
        }
        return 1;
    }
    u32::from(!eras.is_empty())
}

pub(super) fn is_hero_entity(world: &SimWorld, ent: &SimEntity, loaded: &LoadedMod) -> bool {
    if world.trait_stores.heroes.contains_key(&ent.id) {
        return true;
    }
    match &ent.kind {
        EntityKind::Unit { unit_id } => loaded
            .units
            .get(unit_id)
            .is_some_and(|u| u.hero_trait.is_some()),
        EntityKind::Building { .. } => false,
    }
}

pub(super) fn is_combat_unit(ent: &SimEntity, loaded: &LoadedMod) -> bool {
    if ent.health <= 0.0 || ent.transported_by.is_some() {
        return false;
    }
    match &ent.kind {
        EntityKind::Unit { unit_id } => loaded.units.get(unit_id).is_some_and(|u| {
            !matches!(u.role, UnitRole::Harvester | UnitRole::Engineer)
                && u.builder_trait.is_none()
                && (u.primary_weapon.is_some() || u.hero_trait.is_some())
        }),
        EntityKind::Building { .. } => false,
    }
}

/// Refreshes standard sensor keys in `cmdr.blackboard` deterministically (`Task 243-Sim`).
pub fn populate_ai_blackboard(
    world: &SimWorld,
    ai_player_id: u8,
    base_world: [f32; 3],
    loaded: &LoadedMod,
    cmdr: &mut AiCommanderState,
) {
    let Some(econ) = world.players.get(&ai_player_id) else {
        return;
    };
    cmdr.blackboard
        .insert("credits".to_string(), econ.credits as f32);
    for (res_id, &amt) in &econ.resources {
        cmdr.blackboard.insert(res_id.clone(), amt as f32);
        cmdr.blackboard
            .insert(format!("resource.{res_id}"), amt as f32);
    }

    let mut army_count = 0_u32;
    let mut hero_alive = false;
    let mut base_damaged = false;
    let mut intruder_near_base = false;

    for ent in world.entities.values() {
        if ent.health <= 0.0 {
            continue;
        }
        if ent.player_id == ai_player_id {
            if is_combat_unit(ent, loaded) {
                army_count += 1;
            }
            if is_hero_entity(world, ent, loaded) {
                hero_alive = true;
            }
            if matches!(ent.kind, EntityKind::Building { .. })
                && ent.health < ent.max_health * 0.95
                && ent.last_damaged_by_player != 0
            {
                base_damaged = true;
            }
        } else if world.are_hostile(ai_player_id, ent.player_id)
            && ent.transported_by.is_none()
            && det_distance_xz_f32(ent.position, base_world) <= 26.0
        {
            intruder_near_base = true;
        }
    }

    let under_attack = base_damaged || intruder_near_base;
    cmdr.blackboard.insert(
        "under_attack".to_string(),
        if under_attack { 1.0 } else { 0.0 },
    );
    cmdr.blackboard
        .insert("army_count".to_string(), army_count as f32);
    cmdr.blackboard
        .insert("enemy_air_ratio".to_string(), cmdr.enemy_air_ratio);
    cmdr.blackboard
        .insert("enemy_armor_ratio".to_string(), cmdr.enemy_armor_ratio);
    cmdr.blackboard.insert(
        "era_order".to_string(),
        resolve_player_era_order(econ, loaded) as f32,
    );
    let supply_ratio = if econ.supply_cap > 0 {
        (SimScalar::from_num(econ.supply_used as i32) / SimScalar::from_num(econ.supply_cap as i32))
            .to_f32()
    } else {
        0.0
    };
    cmdr.blackboard
        .insert("supply_ratio".to_string(), supply_ratio);
    cmdr.blackboard
        .insert("hero_alive".to_string(), if hero_alive { 1.0 } else { 0.0 });
    cmdr.blackboard
        .insert("stall_efficiency".to_string(), econ.stall_efficiency);
}

pub(super) struct BtEvalContext<'a> {
    pub(super) world: &'a SimWorld,
    pub(super) ai_player_id: u8,
    pub(super) econ: &'a PlayerEconomy,
    pub(super) profile: Option<&'a AiProfileDef>,
    pub(super) _difficulty: AiDifficulty,
    pub(super) base_world: [f32; 3],
    pub(super) staging_world: [f32; 3],
    pub(super) has_conyard: bool,
    pub(super) owned_buildings: &'a BTreeSet<String>,
    pub(super) avail_buildings: &'a [String],
    pub(super) avail_units: Vec<String>,
    pub(super) loaded: &'a LoadedMod,
}

fn evaluate_bt_condition(
    cond: &BlackboardConditionKind,
    ctx: &BtEvalContext<'_>,
    cmdr: &AiCommanderState,
) -> bool {
    match cond {
        BlackboardConditionKind::CreditsAbove { amount } => (ctx.econ.credits as i64) >= *amount,
        BlackboardConditionKind::CreditsBelow { amount } => (ctx.econ.credits as i64) < *amount,
        BlackboardConditionKind::ResourceAbove {
            resource_id,
            amount,
        } => {
            let have = if resource_id == "credits" {
                ctx.econ.credits as i64
            } else {
                ctx.econ.resources.get(resource_id).copied().unwrap_or(0) as i64
            };
            have >= *amount
        }
        BlackboardConditionKind::UnderAttack => {
            cmdr.blackboard.get("under_attack").copied().unwrap_or(0.0) >= 0.5
        }
        BlackboardConditionKind::ArmyCountAtLeast { count } => {
            cmdr.blackboard.get("army_count").copied().unwrap_or(0.0) >= (*count as f32)
        }
        BlackboardConditionKind::EnemyAirRatioAbove { ratio } => cmdr.enemy_air_ratio > *ratio,
        BlackboardConditionKind::EraAtLeast { era_order } => {
            resolve_player_era_order(ctx.econ, ctx.loaded) >= *era_order
        }
        BlackboardConditionKind::HasEra { era_id } => {
            ctx.world
                .has_era_requirement(ctx.ai_player_id, Some(era_id), ctx.loaded)
        }
        BlackboardConditionKind::HasUpgrade { upgrade_id } => {
            ctx.econ.researched_upgrades.contains(upgrade_id)
        }
        BlackboardConditionKind::HasBuilding { building_id } => {
            ctx.owned_buildings.contains(building_id)
        }
        BlackboardConditionKind::SupplyRatioAbove { ratio } => {
            cmdr.blackboard.get("supply_ratio").copied().unwrap_or(0.0) > *ratio
        }
        BlackboardConditionKind::HeroAlive => {
            cmdr.blackboard.get("hero_alive").copied().unwrap_or(0.0) >= 0.5
        }
        BlackboardConditionKind::BlackboardKeyAtLeast { key, threshold } => {
            cmdr.blackboard.get(key).copied().unwrap_or(0.0) >= *threshold
        }
    }
}

fn eval_node_recursive(
    bt: &BehaviorTreeDef,
    node_id: &str,
    depth: usize,
    ctx: &BtEvalContext<'_>,
    cmdr: &mut AiCommanderState,
    commands: &mut Vec<GameCommand>,
) -> (bool, Vec<String>) {
    if depth > 64 {
        return (false, Vec::new());
    }
    let Some(node) = bt.find_node(node_id) else {
        return (false, Vec::new());
    };
    match &node.node_kind {
        BtNodeKind::Selector { children } => {
            for child_id in children {
                let (ok, mut sub_path) =
                    eval_node_recursive(bt, child_id, depth + 1, ctx, cmdr, commands);
                if ok {
                    let mut path = Vec::with_capacity(1 + sub_path.len());
                    path.push(node.id.clone());
                    path.append(&mut sub_path);
                    return (true, path);
                }
            }
            (false, vec![node.id.clone()])
        }
        BtNodeKind::Sequence { children } => {
            let mut path = vec![node.id.clone()];
            for child_id in children {
                let (ok, mut sub_path) =
                    eval_node_recursive(bt, child_id, depth + 1, ctx, cmdr, commands);
                path.append(&mut sub_path);
                if !ok {
                    return (false, path);
                }
            }
            (true, path)
        }
        BtNodeKind::UtilitySelector {
            children,
            utility_weights,
        } => {
            let mut scored: Vec<(i64, usize, &String)> = children
                .iter()
                .enumerate()
                .map(|(idx, cid)| {
                    let base_u = bt.find_node(cid).map(|n| n.base_utility).unwrap_or(1.0);
                    let weight_u = utility_weights.get(cid).copied().unwrap_or(1.0);
                    let bb_bonus = cmdr
                        .blackboard
                        .get(&format!("utility.{cid}"))
                        .copied()
                        .unwrap_or(1.0);
                    let score = SimScalar::from_f32(base_u)
                        * SimScalar::from_f32(weight_u)
                        * SimScalar::from_f32(bb_bonus);
                    (score.0, idx, cid)
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

            for (_, _, child_id) in scored {
                let (ok, mut sub_path) =
                    eval_node_recursive(bt, child_id, depth + 1, ctx, cmdr, commands);
                if ok {
                    let mut path = Vec::with_capacity(1 + sub_path.len());
                    path.push(node.id.clone());
                    path.append(&mut sub_path);
                    return (true, path);
                }
            }
            (false, vec![node.id.clone()])
        }
        BtNodeKind::Inverter { child } => {
            let (ok, mut sub_path) = eval_node_recursive(bt, child, depth + 1, ctx, cmdr, commands);
            let mut path = Vec::with_capacity(1 + sub_path.len());
            path.push(node.id.clone());
            path.append(&mut sub_path);
            (!ok, path)
        }
        BtNodeKind::Cooldown {
            cooldown_secs,
            child,
        } => {
            if let Some(&until_tick) = cmdr.bt_cooldown_until_tick.get(&node.id) {
                if ctx.world.tick < until_tick {
                    return (false, vec![node.id.clone()]);
                }
            }
            let (ok, mut sub_path) = eval_node_recursive(bt, child, depth + 1, ctx, cmdr, commands);
            if ok {
                let cd_ticks = ((*cooldown_secs / ctx.world.dt.max(0.001)).round().max(1.0)) as u64;
                cmdr.bt_cooldown_until_tick
                    .insert(node.id.clone(), ctx.world.tick.saturating_add(cd_ticks));
            }
            let mut path = Vec::with_capacity(1 + sub_path.len());
            path.push(node.id.clone());
            path.append(&mut sub_path);
            (ok, path)
        }
        BtNodeKind::BlackboardCondition { condition } => {
            let ok = evaluate_bt_condition(condition, ctx, cmdr);
            (ok, vec![node.id.clone()])
        }
        BtNodeKind::AiAction { action } => {
            let ok = execute_bt_action(action, ctx, cmdr, commands);
            (ok, vec![node.id.clone()])
        }
    }
}

/// Evaluates an acyclic `BehaviorTreeDef` deterministically against `cmdr.blackboard`,
/// recording `cmdr.active_node_path` for live Workbench inspection and emitting `GameCommand`s (`Task 243-Sim`).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_profile_behavior_tree(
    bt: &BehaviorTreeDef,
    world: &SimWorld,
    ai_player_id: u8,
    econ: &PlayerEconomy,
    profile: Option<&AiProfileDef>,
    difficulty: AiDifficulty,
    base_world: [f32; 3],
    staging_world: [f32; 3],
    has_conyard: bool,
    owned_buildings: &BTreeSet<String>,
    avail_buildings: &[String],
    loaded: &LoadedMod,
    cmdr: &mut AiCommanderState,
    commands: &mut Vec<GameCommand>,
) -> bool {
    if bt.root_node.is_empty() || bt.nodes.is_empty() || bt.has_cycle() {
        cmdr.active_node_path.clear();
        return false;
    }
    let avail_units = world.available_units_for_player(ai_player_id, loaded);
    let ctx = BtEvalContext {
        world,
        ai_player_id,
        econ,
        profile,
        _difficulty: difficulty,
        base_world,
        staging_world,
        has_conyard,
        owned_buildings,
        avail_buildings,
        avail_units,
        loaded,
    };
    let (ok, path) = eval_node_recursive(bt, &bt.root_node, 0, &ctx, cmdr, commands);
    cmdr.active_node_path = path;
    ok
}
