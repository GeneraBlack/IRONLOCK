pub mod base_planner;
pub mod behavior_tree;
mod behavior_tree_actions;
pub mod influence_map;
mod tactics;

pub use base_planner::{BaseLayoutPlanner, PlannedFortification};
pub use behavior_tree::AiBlackboard;
pub use influence_map::{AiCommanderState, InfluenceMap};
use tactics::*;

use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    AiProfileDef, BuildingDef, BuildingRole, MovementDomain, SupportPowerKind, TriggerAction,
    UnitRole,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AiDifficulty {
    Easy,
    #[default]
    Medium,
    Hard,
    Brutal,
}

impl AiDifficulty {
    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "EASY",
            Self::Medium => "MEDIUM",
            Self::Hard => "HARD",
            Self::Brutal => "BRUTAL",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Easy => Self::Medium,
            Self::Medium => Self::Hard,
            Self::Hard => Self::Brutal,
            Self::Brutal => Self::Easy,
        }
    }

    pub fn decision_interval_ticks(self) -> u64 {
        match self {
            Self::Easy => 40,
            Self::Medium => 20,
            Self::Hard | Self::Brutal => 10,
        }
    }

    pub fn grace_period_ticks(self) -> u64 {
        match self {
            Self::Easy => 1400,
            Self::Medium => 900,
            Self::Hard => 600,
            Self::Brutal => 400,
        }
    }

    pub fn wave_interval_ticks(self) -> u64 {
        match self {
            Self::Easy => 760,
            Self::Medium => 500,
            Self::Hard => 340,
            Self::Brutal => 240,
        }
    }

    pub fn garrison_count(self) -> usize {
        match self {
            Self::Easy => 4,
            Self::Medium | Self::Hard => 3,
            Self::Brutal => 2,
        }
    }

    pub fn min_wave_size(self) -> usize {
        match self {
            Self::Easy => 4,
            Self::Medium | Self::Hard => 5,
            Self::Brutal => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AiPersonality {
    #[default]
    Balanced,
    Rusher,
    Turtle,
    AirForce,
    Tech,
}

impl AiPersonality {
    pub const ALL: [Self; 5] = [
        Self::Balanced,
        Self::Rusher,
        Self::Turtle,
        Self::AirForce,
        Self::Tech,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "BALANCED",
            Self::Rusher => "RUSHER",
            Self::Turtle => "TURTLE",
            Self::AirForce => "AIR MARSHAL",
            Self::Tech => "TECH",
        }
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Rusher => "rusher",
            Self::Turtle => "turtle",
            Self::AirForce => "air_marshal",
            Self::Tech => "tech",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Balanced => Self::Rusher,
            Self::Rusher => Self::Turtle,
            Self::Turtle => Self::AirForce,
            Self::AirForce => Self::Tech,
            Self::Tech => Self::Balanced,
        }
    }
}

fn personality_keys_match(profile_key: &str, requested_key: &str) -> bool {
    if profile_key.eq_ignore_ascii_case(requested_key) {
        return true;
    }
    let is_air_a = profile_key.eq_ignore_ascii_case("air_marshal")
        || profile_key.eq_ignore_ascii_case("air_force");
    let is_air_b = requested_key.eq_ignore_ascii_case("air_marshal")
        || requested_key.eq_ignore_ascii_case("air_force");
    is_air_a && is_air_b
}

/// Resolves a data-driven `AiProfileDef` from `LoadedMod` by exact profile `id`, or `(faction, personality_id)`,
/// falling back to any faction-compatible profile (`Task 216`).
pub fn resolve_ai_profile<'a>(
    loaded: &'a LoadedMod,
    faction: &str,
    personality_id: &str,
) -> Option<&'a AiProfileDef> {
    if let Some(exact) = loaded.ai_profiles.get(personality_id) {
        return Some(exact);
    }
    let faction_matches = |p_fac: &str| {
        p_fac.eq_ignore_ascii_case(faction)
            || p_fac.eq_ignore_ascii_case("any")
            || p_fac.eq_ignore_ascii_case("shared")
    };
    loaded
        .ai_profiles
        .values()
        .find(|p| {
            p.faction.eq_ignore_ascii_case(faction)
                && (personality_keys_match(p.effective_personality_id(), personality_id)
                    || personality_keys_match(&p.personality, personality_id))
        })
        .or_else(|| {
            loaded.ai_profiles.values().find(|p| {
                faction_matches(&p.faction)
                    && (personality_keys_match(p.effective_personality_id(), personality_id)
                        || personality_keys_match(&p.personality, personality_id))
            })
        })
        .or_else(|| {
            loaded.ai_profiles.values().find(|p| {
                p.faction.eq_ignore_ascii_case(faction)
                    && p.effective_personality_id()
                        .eq_ignore_ascii_case("balanced")
            })
        })
        .or_else(|| {
            loaded
                .ai_profiles
                .values()
                .find(|p| p.faction.eq_ignore_ascii_case(faction))
        })
}

fn build_ai_script_snapshots(
    world: &SimWorld,
) -> (
    BTreeMap<u8, rts_sdk::scripting::PlayerResourceSnapshot>,
    Vec<rts_sdk::scripting::EntityQuerySnapshot>,
) {
    let mut counts_by_player: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
    let mut entity_snapshots = Vec::new();
    for e in world.entities.values() {
        if e.health <= 0.0 {
            continue;
        }
        let entry = counts_by_player.entry(e.player_id).or_insert((0, 0));
        let is_building = matches!(e.kind, EntityKind::Building { .. });
        if is_building {
            entry.1 += 1;
        } else {
            entry.0 += 1;
        }
        if e.transported_by.is_none() {
            entity_snapshots.push(rts_sdk::scripting::EntityQuerySnapshot {
                entity_id: e.id,
                def_id: e.def_id().to_string(),
                player_id: e.player_id,
                is_building,
                tile_pos: e.tile_origin,
                health: e.health,
                max_health: e.max_health,
            });
        }
    }
    let mut player_snapshots = BTreeMap::new();
    for (&pid, econ) in &world.players {
        let (unit_count, building_count) = counts_by_player.get(&pid).copied().unwrap_or((0, 0));
        let mut res_snap = econ.resources.clone();
        res_snap.insert("credits".to_string(), econ.credits);
        player_snapshots.insert(
            pid,
            rts_sdk::scripting::PlayerResourceSnapshot {
                player_id: pid,
                credits: econ.credits,
                power_balance: econ.net_power(),
                resources: res_snap,
                unit_count,
                building_count,
            },
        );
    }
    (player_snapshots, entity_snapshots)
}

pub(crate) fn emit_ai_place_or_worker_build(
    world: &SimWorld,
    ai_player_id: u8,
    b_def: &BuildingDef,
    preferred_tile: Option<[i32; 2]>,
    has_conyard: bool,
    loaded: &LoadedMod,
    commands: &mut Vec<GameCommand>,
) -> bool {
    let place_opt = preferred_tile
        .filter(|&t| {
            if b_def.water_bound {
                world.grid.in_bounds(t)
                    && world.grid.water_tiles.contains(&t)
                    && world.is_within_build_radius(ai_player_id, t, b_def.footprint)
            } else {
                BaseLayoutPlanner::is_valid_site(
                    world,
                    ai_player_id,
                    b_def.role,
                    t,
                    b_def.footprint,
                    loaded,
                )
            }
        })
        .or_else(|| {
            if b_def.water_bound {
                find_water_build_spot_near_base(world, ai_player_id, b_def.footprint, loaded)
            } else {
                find_build_spot_for_role(world, ai_player_id, b_def.role, b_def.footprint, loaded)
            }
        });
    let Some(place_tile) = place_opt else {
        return false;
    };
    let worker_builder_id = if !has_conyard {
        world.entities.values().find_map(|e| {
            if e.player_id != ai_player_id
                || e.health <= 0.0
                || e.transported_by.is_some()
                || e.worker_build_target_id.is_some()
            {
                return None;
            }
            match &e.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .and_then(|u| u.builder_trait.as_ref())
                    .filter(|bt| bt.can_build(&b_def.id))
                    .map(|_| e.id),
                _ => None,
            }
        })
    } else {
        None
    };
    if let Some(w_id) = worker_builder_id {
        commands.push(GameCommand::OrderWorkerConstruct {
            player_id: ai_player_id,
            worker_ids: vec![w_id],
            building_id: b_def.id.clone(),
            tile_pos: place_tile,
        });
    } else {
        commands.push(GameCommand::PlaceBuilding {
            player_id: ai_player_id,
            building_id: b_def.id.clone(),
            tile_pos: place_tile,
        });
    }
    true
}

/// Generates deterministic `GameCommand`s for an AI-controlled player using `AiDifficulty::Medium`.
pub fn generate_ai_commands(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
) -> Vec<GameCommand> {
    generate_ai_commands_with_difficulty(world, ai_player_id, loaded, AiDifficulty::Medium)
}

/// Generates deterministic `GameCommand`s for an AI-controlled player on the current tick
/// according to the selected `AiDifficulty`.
pub fn generate_ai_commands_with_difficulty(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
) -> Vec<GameCommand> {
    let configured_key = world
        .ai_commanders
        .get(&ai_player_id)
        .and_then(|c| {
            c.configured_personality_id
                .clone()
                .or_else(|| c.configured_personality.map(|p| p.config_key().to_string()))
        })
        .unwrap_or_else(|| AiPersonality::Balanced.config_key().to_string());
    generate_ai_commands_with_personality_id(
        world,
        ai_player_id,
        loaded,
        difficulty,
        &configured_key,
    )
}

/// Generates deterministic `GameCommand`s for an AI-controlled player on the current tick
/// according to the selected `AiDifficulty` and `AiPersonality`.
pub fn generate_ai_commands_with_profile(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
    personality: AiPersonality,
) -> Vec<GameCommand> {
    let configured_key = world
        .ai_commanders
        .get(&ai_player_id)
        .and_then(|c| c.configured_personality_id.as_deref())
        .unwrap_or_else(|| personality.config_key());
    generate_ai_commands_with_personality_id(
        world,
        ai_player_id,
        loaded,
        difficulty,
        configured_key,
    )
}

/// Steps an AI commander directly on `&mut SimWorld`, persisting `AiCommanderState` intelligence
/// as well as any `world_custom_flags` / `trigger_vars` mutated by Rhai/Lua `on_ai_tick`, and
/// applying all generated `GameCommand`s (`Tasks 216, 222`).
pub fn step_ai_player_in_world(
    world: &mut SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
    personality_id: Option<&str>,
) {
    let key = personality_id
        .map(|s| s.to_string())
        .or_else(|| {
            world.ai_commanders.get(&ai_player_id).and_then(|c| {
                c.configured_personality_id
                    .clone()
                    .or_else(|| c.configured_personality.map(|p| p.config_key().to_string()))
            })
        })
        .unwrap_or_else(|| "balanced".to_string());

    let mut mutated_flags: Option<BTreeMap<String, bool>> = None;
    let mut mutated_vars: Option<BTreeMap<String, i64>> = None;
    let mut updated_cmdr: Option<AiCommanderState> = None;

    let cmds = generate_ai_commands_internal(
        world,
        ai_player_id,
        loaded,
        difficulty,
        &key,
        &mut mutated_flags,
        &mut mutated_vars,
        &mut updated_cmdr,
    );
    if let Some(flags) = mutated_flags {
        world.world_custom_flags = flags;
    }
    if let Some(vars) = mutated_vars {
        world.trigger_vars = vars;
    }
    if let Some(cmdr) = updated_cmdr {
        world.ai_commanders.insert(ai_player_id, cmdr);
    }
    for cmd in cmds {
        world.apply_command(cmd, loaded);
    }
}

/// Generates deterministic `GameCommand`s for an AI-controlled player using an arbitrary moddable
/// `personality_id` (e.g., `"balanced"`, `"rusher"`, `"turtle"`, `"air_marshal"`, `"tech"`, or a custom `AiProfileDef.id`).
pub fn generate_ai_commands_with_personality_id(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
    personality_id: &str,
) -> Vec<GameCommand> {
    let mut unused_flags = None;
    let mut unused_vars = None;
    let mut unused_cmdr = None;
    generate_ai_commands_internal(
        world,
        ai_player_id,
        loaded,
        difficulty,
        personality_id,
        &mut unused_flags,
        &mut unused_vars,
        &mut unused_cmdr,
    )
}

#[allow(clippy::too_many_arguments)]
fn generate_ai_commands_internal(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
    difficulty: AiDifficulty,
    personality_id: &str,
    out_flags: &mut Option<BTreeMap<String, bool>>,
    out_vars: &mut Option<BTreeMap<String, i64>>,
    out_cmdr: &mut Option<AiCommanderState>,
) -> Vec<GameCommand> {
    if !world
        .tick
        .is_multiple_of(difficulty.decision_interval_ticks())
    {
        return Vec::new();
    }

    let Some(econ) = world.players.get(&ai_player_id) else {
        return Vec::new();
    };

    let active_profile = resolve_ai_profile(loaded, &econ.faction, personality_id);
    let eff_personality_key = active_profile
        .map(|p| p.effective_personality_id())
        .unwrap_or(personality_id);
    let is_rusher = eff_personality_key.eq_ignore_ascii_case("rusher");
    let is_turtle = eff_personality_key.eq_ignore_ascii_case("turtle");
    let is_air = personality_keys_match(eff_personality_key, "air_marshal");
    let is_tech = eff_personality_key.eq_ignore_ascii_case("tech");

    let mut commands = Vec::new();
    let owned_buildings = world.owned_building_ids(ai_player_id);
    let avail_buildings = world.available_buildings_for_player(ai_player_id, loaded);

    let has_conyard_early = owned_buildings.iter().any(|ob_id| {
        loaded
            .buildings
            .get(ob_id)
            .is_some_and(|b| b.role == BuildingRole::ConstructionYard)
    });

    let base_tile = resolve_player_base_tile(world, ai_player_id, loaded);
    let base_world = world.grid.tile_to_world(base_tile);
    let staging_world = compute_staging_rally_point(world, base_world);
    let mut cmdr_intel = {
        let mut st = world
            .ai_commanders
            .get(&ai_player_id)
            .cloned()
            .unwrap_or_else(|| {
                AiCommanderState::new(ai_player_id, world.grid.width, world.grid.height)
            });
        st.configured_difficulty = Some(difficulty);
        if st.configured_personality_id.is_none() {
            st.configured_personality_id = Some(eff_personality_key.to_string());
        }
        st.refresh_intelligence(world, loaded);
        behavior_tree::populate_ai_blackboard(world, ai_player_id, base_world, loaded, &mut st);
        st
    };
    *out_cmdr = Some(cmdr_intel.clone());

    // 0. Acyclic Behavior Tree & Utility AI evaluation (`Task 243-Sim`)
    if let Some(bt) = active_profile.and_then(|p| p.behavior_tree.as_ref()) {
        if !bt.nodes.is_empty() && !bt.has_cycle() {
            behavior_tree::evaluate_profile_behavior_tree(
                bt,
                world,
                ai_player_id,
                econ,
                active_profile,
                difficulty,
                base_world,
                staging_world,
                has_conyard_early,
                &owned_buildings,
                &avail_buildings,
                loaded,
                &mut cmdr_intel,
                &mut commands,
            );
            *out_cmdr = Some(cmdr_intel);
            return commands;
        }
    }

    // 0a. Full Rhai/Lua `on_ai_tick` execution with live `player_snapshots` & `entity_snapshots` (`Task 216`)
    if let Some(hook_name) = active_profile.and_then(|p| p.on_ai_tick.as_deref()) {
        if loaded.script_host.has_hook(hook_name) {
            let (player_snapshots, entity_snapshots) = build_ai_script_snapshots(world);
            let mut ev_ctx = rts_sdk::scripting::WorldEventContext {
                event_type: "on_ai_tick".to_string(),
                tick: world.tick,
                player_id: ai_player_id,
                bonus_credits: econ.credits as i32,
                custom_flags: world.world_custom_flags.clone(),
                trigger_vars: world.trigger_vars.clone(),
                player_snapshots,
                entity_snapshots,
                ..Default::default()
            };
            if loaded
                .script_host
                .invoke_world_event(hook_name, &mut ev_ctx)
                .unwrap_or(false)
            {
                if ev_ctx.custom_flags != world.world_custom_flags {
                    *out_flags = Some(ev_ctx.custom_flags);
                }
                if ev_ctx.trigger_vars != world.trigger_vars {
                    *out_vars = Some(ev_ctx.trigger_vars);
                }
                if let Some(ref req_id) = ev_ctx.spawn_unit_id {
                    if let Some(u_def) = loaded.units.get(req_id) {
                        let count = ev_ctx.spawn_count.clamp(1, 5);
                        let mut rem_credits = econ.credits;
                        for _ in 0..count {
                            if rem_credits >= u_def.cost {
                                rem_credits -= u_def.cost;
                                commands.push(GameCommand::ProduceUnit {
                                    player_id: ai_player_id,
                                    producer_entity_id: None,
                                    unit_id: req_id.clone(),
                                });
                            }
                        }
                    } else if let Some(b_def) = loaded.buildings.get(req_id) {
                        let b_costs = b_def.effective_resource_costs();
                        if crate::world::economy::can_afford_costs(econ, &b_costs) {
                            emit_ai_place_or_worker_build(
                                world,
                                ai_player_id,
                                b_def,
                                ev_ctx.spawn_tile,
                                has_conyard_early,
                                loaded,
                                &mut commands,
                            );
                        }
                    } else if let Some(upg_def) = loaded.upgrades.get(req_id) {
                        if econ.credits >= upg_def.cost {
                            commands.push(GameCommand::ResearchUpgrade {
                                player_id: ai_player_id,
                                upgrade_id: req_id.clone(),
                            });
                        }
                    }
                }
                for qa in ev_ctx.queued_actions {
                    match qa {
                        TriggerAction::SpawnBuilding {
                            building_id, tile, ..
                        } => {
                            if let Some(b_def) = loaded.buildings.get(&building_id) {
                                let b_costs = b_def.effective_resource_costs();
                                if crate::world::economy::can_afford_costs(econ, &b_costs) {
                                    emit_ai_place_or_worker_build(
                                        world,
                                        ai_player_id,
                                        b_def,
                                        Some(tile),
                                        has_conyard_early,
                                        loaded,
                                        &mut commands,
                                    );
                                }
                            }
                        }
                        TriggerAction::SpawnReinforcements { unit_id, count, .. } => {
                            if let Some(u_def) = loaded.units.get(&unit_id) {
                                let mut rem = econ.credits;
                                for _ in 0..count.clamp(1, 5) {
                                    if rem >= u_def.cost {
                                        rem -= u_def.cost;
                                        commands.push(GameCommand::ProduceUnit {
                                            player_id: ai_player_id,
                                            producer_entity_id: None,
                                            unit_id: unit_id.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        TriggerAction::FireSupportPower {
                            power_id,
                            target_tile,
                            ..
                        } => {
                            commands.push(GameCommand::CastSupportPower {
                                player_id: ai_player_id,
                                power_id,
                                source_pos: Some(staging_world),
                                target_pos: world.grid.tile_to_world(target_tile),
                            });
                        }
                        _ => {}
                    }
                }
                if let Some(atk_target) = ev_ctx.attack_move_target {
                    let attacker_ids: Vec<u64> = world
                        .entities
                        .values()
                        .filter_map(|e| {
                            if e.player_id != ai_player_id
                                || !world.can_entity_act(e, loaded)
                                || e.transported_by.is_some()
                            {
                                return None;
                            }
                            match &e.kind {
                                EntityKind::Unit { unit_id } => loaded
                                    .units
                                    .get(unit_id)
                                    .filter(|u| {
                                        matches!(u.role, UnitRole::Infantry | UnitRole::Vehicle)
                                            && u.primary_weapon.is_some()
                                    })
                                    .map(|_| e.id),
                                _ => None,
                            }
                        })
                        .collect();
                    if !attacker_ids.is_empty() {
                        commands.push(GameCommand::AttackMove {
                            player_id: ai_player_id,
                            entity_ids: attacker_ids,
                            target_pos: atk_target,
                        });
                    }
                }
            }
        }
    }

    // Auto-repair damaged AI structures on Hard & Brutal (or Turtle personality)
    if (matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal) || is_turtle)
        && econ.credits >= 250
    {
        for ent in world.entities.values() {
            if ent.player_id == ai_player_id
                && ent.health > 0.0
                && ent.construction_progress >= 1.0
                && !ent.is_phased_out()
                && matches!(ent.kind, EntityKind::Building { .. })
                && ent.health < ent.max_health * 0.85
                && !ent.is_repairing
            {
                commands.push(GameCommand::ToggleRepairBuilding {
                    player_id: ai_player_id,
                    building_id: ent.id,
                });
            }
        }
    }

    // 0b. In-place Building Upgrades & Smart Cluster-Scored Superweapons (`Task 216`)
    if difficulty != AiDifficulty::Easy && (econ.net_power() < 50 || econ.credits >= 1200) {
        if let Some(upg_b_id) = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && e.construction_progress >= 1.0
                    && e.upgrading_to.is_none()
            })
            .find_map(|e| match &e.kind {
                EntityKind::Building { building_id } => {
                    let b_def = loaded.buildings.get(building_id)?;
                    let target_id = b_def.upgrades_to.as_ref()?;
                    let target_def = loaded.buildings.get(target_id)?;
                    let cost = if b_def.upgrade_cost > 0 {
                        b_def.upgrade_cost
                    } else {
                        target_def.cost.saturating_sub(b_def.cost).max(200)
                    };
                    if econ.credits >= cost {
                        Some(e.id)
                    } else {
                        None
                    }
                }
                _ => None,
            })
        {
            commands.push(GameCommand::UpgradeBuilding {
                player_id: ai_player_id,
                building_id: upg_b_id,
            });
        }
    }

    if difficulty != AiDifficulty::Easy {
        let mut avail_powers = world.available_support_powers_for_player(ai_player_id, loaded);
        let sw_doctrine = active_profile
            .and_then(|p| p.superweapon_doctrine.as_deref())
            .unwrap_or("balanced");
        if let Some(prof) = active_profile {
            if !prof.preferred_superweapons.is_empty() {
                avail_powers.sort_by_key(|(sp_def, _)| {
                    prof.preferred_superweapons
                        .iter()
                        .position(|pref| pref == &sp_def.id)
                        .unwrap_or(usize::MAX)
                });
            }
        }
        for (sp_def, rem_cd) in avail_powers {
            if rem_cd > 0.0 {
                continue;
            }
            let cast_plan: Option<(Option<[f32; 3]>, [f32; 3])> = match sp_def.kind {
                SupportPowerKind::ForceShield => {
                    // ONLY cast ForceShield on high-value friendly structures actively under attack / damaged!
                    let shield_r = sp_def.radius.max(6.0);
                    world
                        .entities
                        .values()
                        .filter_map(|e| {
                            if e.player_id != ai_player_id
                                || e.health <= 0.0
                                || e.construction_progress < 1.0
                                || e.is_invulnerable()
                            {
                                return None;
                            }
                            let EntityKind::Building { ref building_id } = e.kind else {
                                return None;
                            };
                            let b_def = loaded.buildings.get(building_id)?;
                            if matches!(
                                b_def.role,
                                BuildingRole::Wall
                                    | BuildingRole::Gate
                                    | BuildingRole::Bridge
                                    | BuildingRole::BridgeRepairHut
                            ) {
                                return None;
                            }
                            let recently_damaged =
                                e.health < e.max_health * 0.92 && e.last_damaged_by_player != 0;
                            let enemy_attacking_now = world.entities.values().any(|other| {
                                world.are_hostile(ai_player_id, other.player_id)
                                    && other.health > 0.0
                                    && (other.target_entity_id == Some(e.id)
                                        || (recently_damaged
                                            && distance_2d(other.position, e.position) <= 22.0))
                            }) || world.projectiles.iter().any(|p| {
                                world.are_hostile(ai_player_id, p.attacker_player)
                                    && (p.target_id == e.id
                                        || distance_2d(p.target_pos, e.position) <= 6.0)
                            });
                            if !enemy_attacking_now {
                                return None;
                            }
                            let cluster_val: u32 = world
                                .entities
                                .values()
                                .filter(|f| {
                                    f.player_id == ai_player_id
                                        && f.health > 0.0
                                        && matches!(f.kind, EntityKind::Building { .. })
                                        && distance_2d(f.position, e.position) <= shield_r
                                })
                                .map(|f| match &f.kind {
                                    EntityKind::Building { building_id: fid } => loaded
                                        .buildings
                                        .get(fid)
                                        .map(|fb| {
                                            let role_v = match fb.role {
                                                BuildingRole::ConstructionYard => 3000,
                                                BuildingRole::TechCenter => 2400,
                                                BuildingRole::WarFactory => 2000,
                                                BuildingRole::Refinery => 1600,
                                                _ if fb.superweapon.is_some() => 2800,
                                                _ => 600,
                                            };
                                            role_v + fb.cost
                                        })
                                        .unwrap_or(400),
                                    _ => 0,
                                })
                                .sum();
                            Some(((cluster_val, std::cmp::Reverse(e.id)), e.position))
                        })
                        .max_by_key(|(k, _)| *k)
                        .map(|(_, pos)| (Some(staging_world), pos))
                }
                SupportPowerKind::ChronoShift => {
                    // ONLY Chronoshift pure friendly combat VEHICLE groups (ZERO friendly infantry in pickup radius!)
                    let shift_r = sp_def.radius.max(5.5);
                    let safe_vehicle_source = world
                        .entities
                        .values()
                        .filter_map(|v| {
                            if v.player_id != ai_player_id
                                || !world.can_entity_act(v, loaded)
                                || v.transported_by.is_some()
                            {
                                return None;
                            }
                            let EntityKind::Unit { ref unit_id } = v.kind else {
                                return None;
                            };
                            let u_def = loaded.units.get(unit_id)?;
                            if u_def.role != UnitRole::Vehicle
                                || u_def.primary_weapon.is_none()
                                || u_def.transforms_into_building.is_some()
                                || !matches!(
                                    u_def.movement_domain,
                                    MovementDomain::Ground | MovementDomain::Amphibious
                                )
                            {
                                return None;
                            }
                            // Reject strictly if ANY friendly infantry/engineer is inside pickup blast!
                            let has_friendly_infantry_nearby =
                                world.entities.values().any(|ally| {
                                    ally.player_id == ai_player_id
                                        && ally.health > 0.0
                                        && ally.transported_by.is_none()
                                        && distance_2d(ally.position, v.position) <= shift_r + 1.0
                                        && match &ally.kind {
                                            EntityKind::Unit { unit_id: a_uid } => {
                                                loaded.units.get(a_uid).is_some_and(|au| {
                                                    matches!(
                                                        au.role,
                                                        UnitRole::Infantry | UnitRole::Engineer
                                                    )
                                                })
                                            }
                                            _ => false,
                                        }
                                });
                            if has_friendly_infantry_nearby {
                                return None;
                            }
                            let veh_cluster: u32 = world
                                .entities
                                .values()
                                .filter(|ally| {
                                    ally.player_id == ai_player_id
                                        && ally.health > 0.0
                                        && ally.transported_by.is_none()
                                        && distance_2d(ally.position, v.position) <= shift_r
                                        && match &ally.kind {
                                            EntityKind::Unit { unit_id: a_uid } => {
                                                loaded.units.get(a_uid).is_some_and(|au| {
                                                    au.role == UnitRole::Vehicle
                                                        && au.primary_weapon.is_some()
                                                        && au.transforms_into_building.is_none()
                                                })
                                            }
                                            _ => false,
                                        }
                                })
                                .map(|ally| match &ally.kind {
                                    EntityKind::Unit { unit_id: a_uid } => loaded
                                        .units
                                        .get(a_uid)
                                        .map(|u| u.cost.max(400))
                                        .unwrap_or(400),
                                    _ => 0,
                                })
                                .sum();
                            Some(((veh_cluster, std::cmp::Reverse(v.id)), v.position))
                        })
                        .max_by_key(|(k, _)| *k)
                        .map(|(_, pos)| pos);

                    let enemy_dest = world
                        .entities
                        .values()
                        .filter(|e| {
                            tactics::is_ai_valid_enemy_target(
                                world,
                                loaded,
                                ai_player_id,
                                difficulty,
                                e,
                            )
                        })
                        .max_by_key(|e| {
                            let val = score_superweapon_enemy_entity(e, sw_doctrine, loaded);
                            (val, std::cmp::Reverse(e.id))
                        })
                        .map(|e| {
                            let near_walk = world.grid.find_nearest_walkable([
                                e.tile_origin[0] + 2,
                                e.tile_origin[1] + 2,
                            ]);
                            world.grid.tile_to_world(near_walk)
                        });

                    match (safe_vehicle_source, enemy_dest) {
                        (Some(src), Some(dst)) => Some((Some(src), dst)),
                        _ => None,
                    }
                }
                SupportPowerKind::IronCurtain => {
                    let shield_r = sp_def.radius.max(5.0);
                    world
                        .entities
                        .values()
                        .filter_map(|v| {
                            if v.player_id != ai_player_id
                                || v.health <= 0.0
                                || v.transported_by.is_some()
                            {
                                return None;
                            }
                            let EntityKind::Unit { ref unit_id } = v.kind else {
                                return None;
                            };
                            let u_def = loaded.units.get(unit_id)?;
                            if u_def.role != UnitRole::Vehicle || u_def.primary_weapon.is_none() {
                                return None;
                            }
                            // Never cast Iron Curtain onto a mixed group containing friendly infantry!
                            let hits_friendly_inf = world.entities.values().any(|ally| {
                                ally.player_id == ai_player_id
                                    && ally.health > 0.0
                                    && ally.transported_by.is_none()
                                    && distance_2d(ally.position, v.position) <= shield_r + 0.5
                                    && match &ally.kind {
                                        EntityKind::Unit { unit_id: a_uid } => {
                                            loaded.units.get(a_uid).is_some_and(|au| {
                                                matches!(
                                                    au.role,
                                                    UnitRole::Infantry | UnitRole::Engineer
                                                )
                                            })
                                        }
                                        _ => false,
                                    }
                            });
                            if hits_friendly_inf {
                                return None;
                            }
                            Some(((u_def.cost, std::cmp::Reverse(v.id)), v.position))
                        })
                        .max_by_key(|(k, _)| *k)
                        .map(|(_, pos)| (Some(staging_world), pos))
                }
                _ => {
                    // Cluster-scored offensive superweapon targeting (avoids friendly fire)
                    let blast_r = sp_def.radius.max(6.5);
                    world
                        .entities
                        .values()
                        .filter(|e| {
                            tactics::is_ai_valid_enemy_target(
                                world,
                                loaded,
                                ai_player_id,
                                difficulty,
                                e,
                            )
                        })
                        .filter_map(|cand| {
                            let mut cluster_score: i64 = 0;
                            for other in world.entities.values() {
                                if other.health <= 0.0 || other.transported_by.is_some() {
                                    continue;
                                }
                                if distance_2d(other.position, cand.position) > blast_r {
                                    continue;
                                }
                                if world.are_hostile(ai_player_id, other.player_id) {
                                    if sp_def.kind == SupportPowerKind::GeneticMutator {
                                        if let EntityKind::Unit { ref unit_id } = other.kind {
                                            if loaded.units.get(unit_id).is_some_and(|u| {
                                                matches!(
                                                    u.role,
                                                    UnitRole::Infantry | UnitRole::Engineer
                                                )
                                            }) {
                                                cluster_score += 900;
                                            }
                                        }
                                    } else {
                                        cluster_score += score_superweapon_enemy_entity(
                                            other,
                                            sw_doctrine,
                                            loaded,
                                        )
                                            as i64;
                                    }
                                } else if other.player_id == ai_player_id {
                                    cluster_score -= 1800;
                                }
                            }
                            (cluster_score > 0).then_some((
                                (cluster_score, std::cmp::Reverse(cand.id)),
                                cand.position,
                            ))
                        })
                        .max_by_key(|(k, _)| *k)
                        .map(|(_, pos)| (Some(staging_world), pos))
                }
            };
            if let Some((src_opt, target_pos)) = cast_plan {
                commands.push(GameCommand::CastSupportPower {
                    player_id: ai_player_id,
                    power_id: sp_def.id,
                    source_pos: src_opt,
                    target_pos,
                });
                break;
            }
        }
    }

    // 0c. CommanderPowerTree Node Unlocks & Tech Upgrades (`Tasks 214, 216`)
    if econ.promotion_points > 0 {
        if let Some(tree) = world.commander_tree_for_player(ai_player_id, loaded) {
            let unlocked = world
                .unlocked_commander_nodes
                .get(&ai_player_id)
                .cloned()
                .unwrap_or_default();
            if let Some(node) = tree.nodes.iter().find(|n| {
                let req_rank = n.required_rank.clamp(1, 5);
                let cost_pts = n.cost_points.max(1);
                !unlocked.contains(&n.id)
                    && econ.commander_rank >= req_rank
                    && econ.promotion_points >= cost_pts
                    && n.prerequisites.iter().all(|p| unlocked.contains(p))
            }) {
                commands.push(GameCommand::UnlockCommanderNode {
                    player_id: ai_player_id,
                    node_id: node.id.clone(),
                });
            }
        }
    }
    if econ.active_upgrade.is_none() {
        let avail_upgrades = world.available_upgrades_for_player(ai_player_id, loaded);
        if let Some(upg) = avail_upgrades
            .into_iter()
            .filter_map(|uid| loaded.upgrades.get(&uid))
            .find(|u| {
                if u.requires_promotion_point {
                    econ.promotion_points > 0 && econ.credits >= u.cost
                } else {
                    difficulty != AiDifficulty::Easy && econ.credits >= u.cost.max(1500)
                }
            })
        {
            commands.push(GameCommand::ResearchUpgrade {
                player_id: ai_player_id,
                upgrade_id: upg.id.clone(),
            });
        }
    }

    // 1. Base Construction Logic (data-driven `AiProfileDef.build_order` + ServiceDepot + AA/Ground Defenses + Walls/Gates)
    let has_war_factory = owned_buildings.iter().any(|ob_id| {
        loaded
            .buildings
            .get(ob_id)
            .is_some_and(|b| b.role == BuildingRole::WarFactory)
    });
    let has_service_depot = owned_buildings.iter().any(|ob_id| {
        loaded.buildings.get(ob_id).is_some_and(|b| {
            !b.water_bound
                && (b.role == BuildingRole::ServiceDepot
                    || b.support_aura.repairs_vehicles_per_sec > 0.0)
        })
    });

    let owns_power_plant = owned_buildings.iter().any(|ob_id| {
        loaded
            .buildings
            .get(ob_id)
            .is_some_and(|b| b.role == BuildingRole::PowerPlant)
    });
    let power_plant_needed = if econ.net_power() < 40 || !owns_power_plant {
        avail_buildings
            .iter()
            .filter_map(|b_id| loaded.buildings.get(b_id))
            .find(|b| b.role == BuildingRole::PowerPlant)
    } else {
        None
    };

    let desired_building: Option<&BuildingDef> = if power_plant_needed.is_some() {
        power_plant_needed
    } else {
        let profile_build_choice = active_profile.and_then(|prof| {
            for (idx, b_id) in prof.build_order.iter().enumerate() {
                if !avail_buildings.contains(b_id) {
                    continue;
                }
                let required_count = prof.build_order[..=idx]
                    .iter()
                    .filter(|id| *id == b_id)
                    .count();
                let owned_count = world
                    .entities
                    .values()
                    .filter(|e| {
                        e.player_id == ai_player_id
                            && e.health > 0.0
                            && matches!(&e.kind, EntityKind::Building { building_id } if building_id == b_id)
                    })
                    .count();
                if owned_count < required_count {
                    if let Some(b_def) = loaded.buildings.get(b_id) {
                        if !(difficulty == AiDifficulty::Easy
                            && b_def.role == BuildingRole::TechCenter)
                        {
                            return Some(b_def);
                        }
                    }
                }
            }
            None
        });

        let priority_roles: &[BuildingRole] = if is_rusher {
            &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::WarFactory,
                BuildingRole::Defense,
            ]
        } else if is_turtle {
            &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::Defense,
                BuildingRole::WarFactory,
                BuildingRole::TechCenter,
            ]
        } else if is_air {
            &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::Airfield,
                BuildingRole::WarFactory,
                BuildingRole::TechCenter,
            ]
        } else if is_tech {
            &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::WarFactory,
                BuildingRole::TechCenter,
                BuildingRole::Defense,
            ]
        } else {
            &[
                BuildingRole::Refinery,
                BuildingRole::WarFactory,
                BuildingRole::Barracks,
                BuildingRole::Defense,
                BuildingRole::TechCenter,
            ]
        };

        let missing_tech = profile_build_choice.or_else(|| {
            priority_roles.iter().find_map(|role| {
                if difficulty == AiDifficulty::Easy && *role == BuildingRole::TechCenter {
                    return None;
                }
                if *role == BuildingRole::ServiceDepot && (!has_war_factory || has_service_depot) {
                    return None;
                }
                if *role == BuildingRole::Defense {
                    let already_def = owned_buildings.iter().any(|ob_id| {
                        loaded
                            .buildings
                            .get(ob_id)
                            .is_some_and(|ob| ob.role == BuildingRole::Defense)
                    });
                    if already_def {
                        return None;
                    }
                    return select_threat_matched_defense(
                        world,
                        ai_player_id,
                        &avail_buildings,
                        &cmdr_intel,
                        loaded,
                    );
                }
                let already_owns_role = owned_buildings.iter().any(|ob_id| {
                    loaded
                        .buildings
                        .get(ob_id)
                        .is_some_and(|ob| ob.role == *role && !ob.water_bound)
                });
                if already_owns_role {
                    return None;
                }
                avail_buildings
                    .iter()
                    .filter_map(|b_id| loaded.buildings.get(b_id))
                    .find(|b| b.role == *role && !b.water_bound && !owned_buildings.contains(&b.id))
            })
        });

        let service_depot_choice = if missing_tech.is_none()
            && difficulty != AiDifficulty::Easy
            && has_war_factory
            && !has_service_depot
            && econ.credits >= 1000
        {
            avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .find(|b| {
                    !b.water_bound
                        && (b.role == BuildingRole::ServiceDepot
                            || b.support_aura.repairs_vehicles_per_sec > 0.0)
                })
        } else {
            None
        };

        let turret_threshold = if is_turtle { 1400 } else { 2400 };
        let owned_refinery_count = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && matches!(
                        &e.kind,
                        EntityKind::Building { building_id }
                            if loaded.buildings.get(building_id).is_some_and(|b| b.role == BuildingRole::Refinery)
                    )
            })
            .count()
            + econ
                .building_queue
                .iter()
                .filter(|item| {
                    loaded
                        .buildings
                        .get(&item.0)
                        .is_some_and(|b| b.role == BuildingRole::Refinery)
                })
                .count();
        let has_tech_center = owned_buildings.iter().any(|ob_id| {
            loaded
                .buildings
                .get(ob_id)
                .is_some_and(|b| b.role == BuildingRole::TechCenter)
        });
        let owned_superweapon_count = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && matches!(
                        &e.kind,
                        EntityKind::Building { building_id }
                            if loaded.buildings.get(building_id).is_some_and(|b| b.superweapon.is_some())
                    )
            })
            .count();
        let naval_yard_choice = if owns_power_plant
            && owned_refinery_count >= 1
            && has_war_factory
            && !world.grid.water_tiles.is_empty()
            && !avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .any(|b| b.water_bound && owned_buildings.contains(&b.id))
        {
            avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .find(|b| {
                    b.water_bound
                        && find_water_build_spot_near_base(world, ai_player_id, b.footprint, loaded)
                            .is_some()
                })
        } else {
            None
        };
        let owned_conyard_count = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && matches!(
                        &e.kind,
                        EntityKind::Building { building_id }
                            if loaded.buildings.get(building_id).is_some_and(|b| b.role == BuildingRole::ConstructionYard)
                    )
            })
            .count();
        let target_refineries = (owned_conyard_count + 1).clamp(2, 3);
        let second_refinery_choice = if missing_tech.is_none()
            && naval_yard_choice.is_none()
            && difficulty != AiDifficulty::Easy
            && owned_refinery_count < target_refineries
            && econ.credits >= 2000
            && !world.grid.ore_tiles.is_empty()
        {
            avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .find(|b| b.role == BuildingRole::Refinery && !b.water_bound)
        } else {
            None
        };
        let superweapon_choice = if missing_tech.is_none()
            && naval_yard_choice.is_none()
            && second_refinery_choice.is_none()
            && difficulty != AiDifficulty::Easy
            && has_tech_center
            && owned_superweapon_count == 0
            && econ.credits >= 2500
        {
            let pref_sw = active_profile.and_then(|prof| {
                prof.preferred_superweapons.iter().find_map(|pref| {
                    avail_buildings
                        .iter()
                        .filter_map(|b_id| loaded.buildings.get(b_id))
                        .find(|b| {
                            b.superweapon.is_some()
                                && (&b.id == pref
                                    || b.support_powers.iter().any(|sp| &sp.id == pref))
                        })
                })
            });
            pref_sw.or_else(|| {
                avail_buildings
                    .iter()
                    .filter_map(|b_id| loaded.buildings.get(b_id))
                    .find(|b| b.superweapon.is_some())
            })
        } else {
            None
        };

        if let Some(ny) = naval_yard_choice {
            Some(ny)
        } else if let Some(ref2) = second_refinery_choice {
            Some(ref2)
        } else if let Some(sw) = superweapon_choice {
            Some(sw)
        } else if let Some(sd) = service_depot_choice {
            Some(sd)
        } else if missing_tech.is_none()
            && (matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal) || is_turtle)
            && econ.credits >= turret_threshold
        {
            select_threat_matched_defense(
                world,
                ai_player_id,
                &avail_buildings,
                &cmdr_intel,
                loaded,
            )
        } else {
            missing_tech
        }
    };

    let mut reserved_credits = match difficulty {
        AiDifficulty::Easy => 600u32,
        AiDifficulty::Medium => 250u32,
        AiDifficulty::Hard => 150u32,
        AiDifficulty::Brutal => 0u32,
    };

    let mut placed_primary_structure = false;
    if let Some(b_def) = desired_building {
        let b_costs = b_def.effective_resource_costs();
        if crate::world::economy::can_afford_costs(econ, &b_costs) {
            if emit_ai_place_or_worker_build(
                world,
                ai_player_id,
                b_def,
                None,
                has_conyard_early,
                loaded,
                &mut commands,
            ) {
                placed_primary_structure = true;
                reserved_credits = reserved_credits.max(b_def.cost);
            }
        } else {
            reserved_credits = reserved_credits.max(b_def.cost.min(1600));
        }
    }

    // Ramp & chokepoint Walls/Gates fortification when economy is healthy (`Task 216`)
    if !placed_primary_structure
        && has_conyard_early
        && has_war_factory
        && (is_turtle || matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal))
        && econ.credits >= 1500
    {
        if let Some(fort) = BaseLayoutPlanner::plan_chokepoint_wall_or_gate(
            world,
            ai_player_id,
            base_tile,
            &avail_buildings,
            loaded,
        ) {
            match fort {
                PlannedFortification::Gate {
                    building_id,
                    tile_pos,
                } => {
                    commands.push(GameCommand::PlaceBuilding {
                        player_id: ai_player_id,
                        building_id,
                        tile_pos,
                    });
                }
                PlannedFortification::WallLine {
                    building_id,
                    start_tile,
                    end_tile,
                } => {
                    commands.push(GameCommand::PlaceWallLine {
                        player_id: ai_player_id,
                        building_id,
                        start_tile,
                        end_tile,
                    });
                }
            }
        }
    }

    // 2. Check if the AI's base is under attack (Scramble Defense)
    let base_intruder_id = find_enemy_intruder_near_base(world, ai_player_id, base_world, 28.0);
    if base_intruder_id.is_some() {
        reserved_credits = 0;
    }

    let spendable_credits = econ.credits.saturating_sub(reserved_credits);

    // 3. Data-Driven Unit Production + Amphibious Lift + Secondary MCV Outpost Expansion (`Task 216`)
    let avail_units = world.available_units_for_player(ai_player_id, loaded);
    let has_idle_barracks = world.entities.values().any(|e| {
        e.player_id == ai_player_id
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && e.production_queue.is_empty()
            && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).is_some_and(|b| b.role == BuildingRole::Barracks))
    });
    let has_idle_war_factory = world.entities.values().any(|e| {
        e.player_id == ai_player_id
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && e.production_queue.is_empty()
            && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).is_some_and(|b| matches!(b.role, BuildingRole::WarFactory | BuildingRole::Airfield)))
    });

    let current_infantry_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).is_some_and(|u| u.role == UnitRole::Infantry))
        })
        .count();

    let owned_conyard_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(
                    &e.kind,
                    EntityKind::Building { building_id }
                        if loaded.buildings.get(building_id).is_some_and(|b| b.role == BuildingRole::ConstructionYard)
                )
        })
        .count();
    let has_conyard = owned_conyard_count > 0;
    let owned_mcv = world.entities.values().find_map(|e| {
        if e.player_id != ai_player_id || e.health <= 0.0 || e.transported_by.is_some() {
            return None;
        }
        match &e.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .filter(|u| u.transforms_into_building.is_some())
                .map(|_| (e.id, e.position, e.path_waypoints.is_empty())),
            _ => None,
        }
    });

    let expand_at_credits = active_profile
        .and_then(|p| p.expand_at_credits)
        .unwrap_or(2800)
        .max(1200) as u32;
    let expansion_site_tile = if difficulty != AiDifficulty::Easy && owned_conyard_count < 2 {
        BaseLayoutPlanner::find_distant_expansion_site(
            world,
            ai_player_id,
            base_tile,
            Some(&cmdr_intel.influence),
            loaded,
        )
    } else {
        None
    };

    if !has_conyard {
        if let Some((mcv_id, _, _)) = owned_mcv {
            commands.push(GameCommand::ActivateAbility {
                player_id: ai_player_id,
                entity_ids: vec![mcv_id],
            });
        }
    } else if let Some((mcv_id, mcv_pos, mcv_idle)) = owned_mcv {
        // Drive secondary expansion MCV to distant ore/gem patch and deploy on arrival!
        if let Some(exp_tile) = expansion_site_tile {
            let exp_world = world.grid.tile_to_world(exp_tile);
            if distance_2d(mcv_pos, exp_world) <= 5.5 {
                commands.push(GameCommand::ActivateAbility {
                    player_id: ai_player_id,
                    entity_ids: vec![mcv_id],
                });
            } else if mcv_idle {
                commands.push(GameCommand::MoveUnits {
                    player_id: ai_player_id,
                    entity_ids: vec![mcv_id],
                    target_pos: exp_world,
                });
            }
        } else if distance_2d(mcv_pos, base_world) >= 18.0 {
            commands.push(GameCommand::ActivateAbility {
                player_id: ai_player_id,
                entity_ids: vec![mcv_id],
            });
        }
    }

    let amphibious_enabled = active_profile
        .and_then(|p| p.amphibious_assault_enabled)
        .unwrap_or(true);
    let needs_amphibious_lift = amphibious_enabled
        && enemy_requires_amphibious_lift(world, ai_player_id, base_tile, difficulty, loaded);
    let owned_sealift_transports = count_player_sealift_transports(world, ai_player_id, loaded);

    let current_harvester_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).is_some_and(|u| u.role == UnitRole::Harvester))
        })
        .count();

    let score_unit_by_profile = |u: &rts_sdk::schema::UnitDef| -> (u64, u32) {
        let counter_pct = cmdr_intel.counter_weight_percent(u, loaded);
        let owned_cnt = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == &u.id)
            })
            .count() as u64;
        let base_weight = active_profile
            .and_then(|prof| {
                if prof.unit_weights.is_empty() {
                    None
                } else {
                    Some(prof.unit_weights.get(&u.id).copied().unwrap_or(0) as u64)
                }
            })
            .unwrap_or(10);
        let quota_mult_pct: u64 = active_profile
            .and_then(|prof| {
                if prof.unit_role_quotas.is_empty() {
                    return None;
                }
                let role_key = if matches!(
                    u.movement_domain,
                    MovementDomain::Air | MovementDomain::Jumpjet
                ) {
                    "air"
                } else if u.movement_domain == MovementDomain::Naval {
                    "naval"
                } else if u.role == UnitRole::Infantry {
                    "infantry"
                } else {
                    "vehicle"
                };
                prof.unit_role_quotas
                    .get(role_key)
                    .map(|q| ((q.clamp(0.05, 1.0) * 200.0).round() as u64).max(20))
            })
            .unwrap_or(100);
        let eff_weight = if counter_pct > 100 {
            base_weight.max(15)
        } else {
            base_weight.max(
                if active_profile.is_some_and(|p| p.unit_weights.is_empty()) {
                    10
                } else {
                    0
                },
            )
        };
        if eff_weight > 0 {
            return (
                ((eff_weight * counter_pct * quota_mult_pct) / ((owned_cnt + 1) * 10)),
                u.cost,
            );
        }
        (0, u.cost)
    };

    if has_idle_war_factory {
        let completed_refineries = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && e.construction_progress >= 1.0
                    && matches!(
                        &e.kind,
                        EntityKind::Building { building_id }
                            if loaded.buildings.get(building_id).is_some_and(|b| b.role == BuildingRole::Refinery)
                    )
            })
            .count();
        let desired_harvesters = if difficulty == AiDifficulty::Easy {
            1
        } else {
            (completed_refineries * 2).clamp(2, 5)
        };
        let total_airfield_pads: usize = world
            .entities
            .values()
            .filter_map(|e| {
                if e.player_id != ai_player_id || e.health <= 0.0 || e.construction_progress < 1.0 {
                    return None;
                }
                match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(SimWorld::building_airfield_pad_capacity),
                    _ => None,
                }
            })
            .sum();
        let pad_aircraft_count: usize = world
            .entities
            .values()
            .map(|e| {
                if e.player_id != ai_player_id || e.health <= 0.0 {
                    return 0;
                }
                let is_self_pad_jet = usize::from(
                    world.trait_stores.parent_carrier_id(e).is_none()
                        && matches!(
                            &e.kind,
                            EntityKind::Unit { unit_id }
                                if loaded.units.get(unit_id).is_some_and(|u| {
                                    u.movement_domain == MovementDomain::Air && u.max_ammo > 0
                                })
                        ),
                );
                let queued_pad_jets = e
                    .production_queue
                    .iter()
                    .filter(|item| {
                        loaded.units.get(&item.unit_id).is_some_and(|u| {
                            u.movement_domain == MovementDomain::Air && u.max_ammo > 0
                        })
                    })
                    .count();
                is_self_pad_jet + queued_pad_jets
            })
            .sum();

        let needs_emergency_harvester = current_harvester_count == 0;
        let wants_second_harvester = difficulty != AiDifficulty::Easy
            && current_harvester_count < desired_harvesters
            && spendable_credits >= 1400
            && !world.grid.ore_tiles.is_empty();
        let needs_replacement_mcv = !has_conyard && owned_mcv.is_none();
        let wants_expansion_mcv = has_conyard
            && owned_conyard_count < 2
            && owned_mcv.is_none()
            && expansion_site_tile.is_some()
            && econ.credits >= expand_at_credits;
        let wants_factory_sealift = needs_amphibious_lift && owned_sealift_transports < 2;

        if needs_emergency_harvester || wants_second_harvester {
            let harv_budget = if needs_emergency_harvester {
                econ.credits
            } else {
                spendable_credits
            };
            if let Some(harv) = avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .find(|u| u.role == UnitRole::Harvester && u.cost <= harv_budget)
            {
                commands.push(GameCommand::ProduceUnit {
                    player_id: ai_player_id,
                    producer_entity_id: None,
                    unit_id: harv.id.clone(),
                });
            }
        } else if needs_replacement_mcv || wants_expansion_mcv {
            if let Some(mcv_def) = avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .find(|u| u.transforms_into_building.is_some() && u.cost <= econ.credits)
            {
                commands.push(GameCommand::ProduceUnit {
                    player_id: ai_player_id,
                    producer_entity_id: None,
                    unit_id: mcv_def.id.clone(),
                });
            }
        } else if wants_factory_sealift
            && avail_units.iter().any(|u_id| {
                loaded.units.get(u_id).is_some_and(|u| {
                    u.container.capacity > 0
                        && !u.container.passengers_can_fire
                        && !u.container.adapts_weapon_to_passenger
                        && matches!(
                            u.movement_domain,
                            MovementDomain::Amphibious | MovementDomain::Air
                        )
                        && u.cost <= spendable_credits
                })
            })
        {
            if let Some(trans_def) = avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .filter(|u| {
                    u.container.capacity > 0
                        && !u.container.passengers_can_fire
                        && !u.container.adapts_weapon_to_passenger
                        && matches!(
                            u.movement_domain,
                            MovementDomain::Amphibious | MovementDomain::Air
                        )
                        && u.cost <= spendable_credits
                })
                .max_by_key(|u| (u.container.accepts_vehicles, u.container.capacity))
            {
                commands.push(GameCommand::ProduceUnit {
                    player_id: ai_player_id,
                    producer_entity_id: None,
                    unit_id: trans_def.id.clone(),
                });
            }
        } else if let Some(best_vehicle) = avail_units
            .iter()
            .filter_map(|u_id| loaded.units.get(u_id))
            .filter(|u| {
                let fits_airfield_pad = !(u.movement_domain == MovementDomain::Air
                    && u.max_ammo > 0)
                    || pad_aircraft_count < total_airfield_pads;
                let is_combat_or_carrier = u.primary_weapon.is_some()
                    || u.container.passengers_can_fire
                    || u.container.adapts_weapon_to_passenger;
                u.role == UnitRole::Vehicle
                    && u.movement_domain != MovementDomain::Naval
                    && u.transforms_into_building.is_none()
                    && is_combat_or_carrier
                    && fits_airfield_pad
                    && u.cost <= spendable_credits
                    && (difficulty != AiDifficulty::Easy || u.cost <= 1000)
            })
            .max_by_key(|u| score_unit_by_profile(u))
        {
            commands.push(GameCommand::ProduceUnit {
                player_id: ai_player_id,
                producer_entity_id: None,
                unit_id: best_vehicle.id.clone(),
            });
        }
    }

    let max_infantry = match difficulty {
        AiDifficulty::Easy => 4,
        AiDifficulty::Medium => 6,
        AiDifficulty::Hard => 8,
        AiDifficulty::Brutal => 10,
    };
    let has_neutral_tech_or_broken_bridge = world.entities.values().any(|e| {
        e.player_id == 0
            && e.health > 0.0
            && match &e.kind {
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .is_some_and(|b| b.role != BuildingRole::BridgeRepairHut),
                _ => false,
            }
    }) || !world.destroyed_bridges.is_empty()
        || world
            .grid
            .bridge_seed_tiles
            .iter()
            .any(|t| !world.grid.bridge_tiles.contains(t));
    let current_engineer_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).is_some_and(|u| u.role == UnitRole::Engineer))
        })
        .count();
    let current_spy_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).is_some_and(|u| u.spy_infiltrator))
        })
        .count();

    if has_idle_barracks && has_neutral_tech_or_broken_bridge && current_engineer_count == 0 {
        if let Some(eng) = avail_units
            .iter()
            .filter_map(|u_id| loaded.units.get(u_id))
            .find(|u| u.role == UnitRole::Engineer && u.cost <= spendable_credits)
        {
            commands.push(GameCommand::ProduceUnit {
                player_id: ai_player_id,
                producer_entity_id: None,
                unit_id: eng.id.clone(),
            });
        }
    } else if has_idle_barracks
        && difficulty != AiDifficulty::Easy
        && current_spy_count == 0
        && spendable_credits >= 1400
        && avail_units
            .iter()
            .any(|u_id| loaded.units.get(u_id).is_some_and(|u| u.spy_infiltrator))
    {
        if let Some(spy) = avail_units
            .iter()
            .filter_map(|u_id| loaded.units.get(u_id))
            .find(|u| u.spy_infiltrator && u.cost <= spendable_credits)
        {
            commands.push(GameCommand::ProduceUnit {
                player_id: ai_player_id,
                producer_entity_id: None,
                unit_id: spy.id.clone(),
            });
        }
    } else if has_idle_barracks
        && current_infantry_count < max_infantry
        && (!has_idle_war_factory || difficulty == AiDifficulty::Brutal || needs_amphibious_lift)
    {
        if let Some(best_infantry) = avail_units
            .iter()
            .filter_map(|u_id| loaded.units.get(u_id))
            .filter(|u| {
                u.role == UnitRole::Infantry && !u.spy_infiltrator && u.cost <= spendable_credits
            })
            .max_by_key(|u| score_unit_by_profile(u))
        {
            commands.push(GameCommand::ProduceUnit {
                player_id: ai_player_id,
                producer_entity_id: None,
                unit_id: best_infantry.id.clone(),
            });
        }
    }

    // Naval warship & amphibious/naval sealift transport production when AI owns an idle shipyard
    let has_idle_shipyard = world.entities.values().any(|e| {
        e.player_id == ai_player_id
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && e.production_queue.is_empty()
            && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).is_some_and(|b| b.water_bound))
    });
    if has_idle_shipyard {
        let owned_warships = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && matches!(
                        &e.kind,
                        EntityKind::Unit { unit_id }
                            if loaded.units.get(unit_id).is_some_and(|u| u.movement_domain == MovementDomain::Naval && u.primary_weapon.is_some())
                    )
            })
            .count();
        let wants_sealift_transport = (needs_amphibious_lift
            && (owned_sealift_transports == 0
                || (owned_warships >= 1 && owned_sealift_transports < 2)))
            || (amphibious_enabled && owned_warships >= 3 && owned_sealift_transports == 0);
        let chosen_shipyard_unit = if wants_sealift_transport {
            avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .filter(|u| {
                    matches!(
                        u.movement_domain,
                        MovementDomain::Amphibious | MovementDomain::Naval
                    ) && u.container.capacity > 0
                        && !u.container.passengers_can_fire
                        && !u.container.adapts_weapon_to_passenger
                        && u.cost <= spendable_credits
                })
                .max_by_key(|u| (u.container.accepts_vehicles, u.container.capacity))
        } else {
            None
        }
        .or_else(|| {
            avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .filter(|u| {
                    u.movement_domain == MovementDomain::Naval
                        && u.primary_weapon.is_some()
                        && u.cost <= spendable_credits
                })
                .max_by_key(|u| score_unit_by_profile(u))
        });
        if let Some(naval_unit) = chosen_shipyard_unit {
            commands.push(GameCommand::ProduceUnit {
                player_id: ai_player_id,
                producer_entity_id: None,
                unit_id: naval_unit.id.clone(),
            });
        }
    }

    generate_ai_tactical_commands(
        world,
        ai_player_id,
        loaded,
        difficulty,
        eff_personality_key,
        active_profile,
        needs_amphibious_lift,
        base_world,
        staging_world,
        base_intruder_id,
        &cmdr_intel,
        &mut commands,
    );
    commands
}

fn score_superweapon_enemy_entity(
    e: &crate::world::SimEntity,
    doctrine: &str,
    loaded: &LoadedMod,
) -> u32 {
    let is_econ_doc = doctrine.eq_ignore_ascii_case("economic");
    match &e.kind {
        EntityKind::Building { building_id } => loaded
            .buildings
            .get(building_id)
            .map(|b| {
                let role_bonus = match b.role {
                    BuildingRole::ConstructionYard => 3000,
                    BuildingRole::TechCenter => 2500,
                    BuildingRole::WarFactory => 2000,
                    BuildingRole::Refinery if is_econ_doc => 3200,
                    BuildingRole::Refinery => 1700,
                    _ if b.superweapon.is_some() => 2900,
                    _ => 700,
                };
                role_bonus + b.cost
            })
            .unwrap_or(500),
        EntityKind::Unit { unit_id } => loaded
            .units
            .get(unit_id)
            .map(|u| {
                if is_econ_doc && u.role == UnitRole::Harvester {
                    1800
                } else {
                    u.cost / 2
                }
            })
            .unwrap_or(100),
    }
}

fn select_threat_matched_defense<'a>(
    world: &SimWorld,
    ai_player_id: u8,
    avail_buildings: &[String],
    cmdr_intel: &AiCommanderState,
    loaded: &'a LoadedMod,
) -> Option<&'a BuildingDef> {
    let mut owned_aa_def = 0_usize;
    let mut owned_ground_def = 0_usize;
    for ent in world.entities.values() {
        if ent.player_id != ai_player_id || ent.health <= 0.0 {
            continue;
        }
        if let EntityKind::Building { ref building_id } = ent.kind {
            if let Some(b_def) = loaded.buildings.get(building_id) {
                if b_def.role == BuildingRole::Defense {
                    let (t_air, t_gnd) = b_def
                        .primary_weapon
                        .as_ref()
                        .and_then(|wid| loaded.weapons.get(wid))
                        .map(|w| (w.targets_air, w.targets_ground))
                        .unwrap_or((false, true));
                    if t_air {
                        owned_aa_def += 1;
                    }
                    if t_gnd {
                        owned_ground_def += 1;
                    }
                }
            }
        }
    }
    let wants_aa =
        cmdr_intel.enemy_air_ratio >= 0.22 || (owned_ground_def >= 1 && owned_aa_def == 0);

    avail_buildings
        .iter()
        .filter_map(|b_id| loaded.buildings.get(b_id))
        .filter(|b| b.role == BuildingRole::Defense && !b.water_bound)
        .max_by_key(|b| {
            let (t_air, t_gnd) = b
                .primary_weapon
                .as_ref()
                .and_then(|wid| loaded.weapons.get(wid))
                .map(|w| (w.targets_air, w.targets_ground))
                .unwrap_or((false, true));
            let match_score = if wants_aa {
                if t_air && t_gnd {
                    3
                } else if t_air {
                    2
                } else {
                    0
                }
            } else if t_gnd && t_air {
                3
            } else if t_gnd {
                2
            } else {
                0
            };
            (match_score, b.cost)
        })
}

fn count_player_sealift_transports(
    world: &SimWorld,
    ai_player_id: u8,
    loaded: &LoadedMod,
) -> usize {
    world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(
                    &e.kind,
                    EntityKind::Unit { unit_id }
                        if loaded.units.get(unit_id).is_some_and(|u| {
                            u.container.capacity > 0
                                && !u.container.passengers_can_fire
                                && !u.container.adapts_weapon_to_passenger
                                && matches!(
                                    u.movement_domain,
                                    MovementDomain::Amphibious
                                        | MovementDomain::Naval
                                        | MovementDomain::Air
                                )
                        })
                )
        })
        .count()
}

fn enemy_requires_amphibious_lift(
    world: &SimWorld,
    ai_player_id: u8,
    base_tile: [i32; 2],
    difficulty: AiDifficulty,
    loaded: &LoadedMod,
) -> bool {
    if world.grid.water_tiles.is_empty() {
        return false;
    }
    let Some(enemy_tile) = world
        .entities
        .values()
        .filter(|e| {
            tactics::is_ai_valid_enemy_target(world, loaded, ai_player_id, difficulty, e)
                && e.transported_by.is_none()
                && crate::combat::entity_domain(e, loaded) == MovementDomain::Ground
        })
        .min_by_key(|e| {
            (
                (e.tile_origin[0] - base_tile[0]).abs() + (e.tile_origin[1] - base_tile[1]).abs(),
                e.id,
            )
        })
        .map(|e| e.tile_origin)
    else {
        return false;
    };
    let near_enemy_walk = world.grid.find_nearest_walkable(enemy_tile);
    let near_base_walk = world.grid.find_nearest_walkable(base_tile);
    world
        .grid
        .find_path_for_domain(near_base_walk, near_enemy_walk, MovementDomain::Ground, 0.0)
        .is_empty()
}
