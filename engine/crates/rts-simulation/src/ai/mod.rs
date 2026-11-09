mod tactics;
use tactics::*;

use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingDef, BuildingRole, MovementDomain, UnitRole};
use serde::{Deserialize, Serialize};

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
}

impl AiPersonality {
    pub const ALL: [Self; 4] = [Self::Balanced, Self::Rusher, Self::Turtle, Self::AirForce];

    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "BALANCED",
            Self::Rusher => "RUSHER",
            Self::Turtle => "TURTLE",
            Self::AirForce => "AIR FORCE",
        }
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Rusher => "rusher",
            Self::Turtle => "turtle",
            Self::AirForce => "air_force",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Balanced => Self::Rusher,
            Self::Rusher => Self::Turtle,
            Self::Turtle => Self::AirForce,
            Self::AirForce => Self::Balanced,
        }
    }
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
    generate_ai_commands_with_profile(
        world,
        ai_player_id,
        loaded,
        difficulty,
        AiPersonality::Balanced,
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
    if !world.tick.is_multiple_of(difficulty.decision_interval_ticks()) {
        return Vec::new();
    }

    let Some(econ) = world.players.get(&ai_player_id) else {
        return Vec::new();
    };

    let active_profile = loaded
        .ai_profiles
        .values()
        .find(|p| {
            p.faction.eq_ignore_ascii_case(&econ.faction)
                && p.personality.eq_ignore_ascii_case(personality.config_key())
        })
        .or_else(|| {
            loaded
                .ai_profiles
                .values()
                .find(|p| p.faction.eq_ignore_ascii_case(&econ.faction))
        });

    let mut commands = Vec::new();
    let owned_buildings = world.owned_building_ids(ai_player_id);
    let avail_buildings = world.available_buildings_for_player(ai_player_id, loaded);

    let base_tile = resolve_player_base_tile(world, ai_player_id, loaded);
    let base_world = world.grid.tile_to_world(base_tile);
    let staging_world = compute_staging_rally_point(world, base_world);

    if let Some(hook_name) = active_profile.and_then(|p| p.on_ai_tick.as_deref()) {
        if loaded.script_host.has_hook(hook_name) {
            let mut ev_ctx = rts_sdk::scripting::WorldEventContext {
                event_type: "on_ai_tick".to_string(),
                tick: world.tick,
                player_id: ai_player_id,
                bonus_credits: econ.credits as i32,
                ..Default::default()
            };
            if loaded
                .script_host
                .invoke_world_event(hook_name, &mut ev_ctx)
                .is_ok()
            {
                if let Some(script_unit_id) = ev_ctx.spawn_unit_id {
                    if let Some(u_def) = loaded.units.get(&script_unit_id) {
                        if econ.credits >= u_def.cost {
                            commands.push(GameCommand::ProduceUnit {
                                player_id: ai_player_id,
                                producer_entity_id: None,
                                unit_id: script_unit_id,
                            });
                        }
                    }
                }
            }
        }
    }

    // Auto-repair damaged AI structures on Hard & Brutal (or Turtle personality)
    if (matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal)
        || personality == AiPersonality::Turtle)
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

    // 0b. In-place Building Upgrades (Power Plant Overcharge / Defense Turret Upgrade) & Support Powers
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
        for (sp_def, rem_cd) in world.available_support_powers_for_player(ai_player_id, loaded) {
            if rem_cd <= 0.0 {
                let target_pos_opt = match sp_def.kind {
                    rts_sdk::schema::SupportPowerKind::ForceShield => Some(base_world),
                    rts_sdk::schema::SupportPowerKind::IronCurtain => world
                        .entities
                        .values()
                        .find(|e| {
                            e.player_id == ai_player_id
                                && e.health > 0.0
                                && match &e.kind {
                                    EntityKind::Unit { unit_id } => loaded
                                        .units
                                        .get(unit_id)
                                        .map(|u| u.role == UnitRole::Vehicle)
                                        .unwrap_or(false),
                                    _ => false,
                                }
                        })
                        .map(|e| e.position),
                    _ => world
                        .entities
                        .values()
                        .find(|e| world.are_hostile(ai_player_id, e.player_id) && e.health > 0.0)
                        .map(|e| e.position),
                };
                if let Some(target_pos) = target_pos_opt {
                    commands.push(GameCommand::CastSupportPower {
                        player_id: ai_player_id,
                        power_id: sp_def.id,
                        source_pos: Some(staging_world),
                        target_pos,
                    });
                    break;
                }
            }
        }
    }

    // 0c. Commander Doctrines (Promotion Points) & Tech Upgrades
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

    // 1. Base Construction Logic (with strategic build-order priority per personality & AiProfileDef)
    let desired_building: Option<&BuildingDef> = if econ.net_power() < 40 {
        avail_buildings
            .iter()
            .filter_map(|b_id| loaded.buildings.get(b_id))
            .find(|b| b.role == BuildingRole::PowerPlant)
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

        let priority_roles: &[BuildingRole] = match personality {
            AiPersonality::Rusher => &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::WarFactory,
                BuildingRole::Defense,
            ],
            AiPersonality::Turtle => &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::Defense,
                BuildingRole::WarFactory,
                BuildingRole::TechCenter,
            ],
            AiPersonality::AirForce => &[
                BuildingRole::Refinery,
                BuildingRole::Barracks,
                BuildingRole::Airfield,
                BuildingRole::WarFactory,
                BuildingRole::TechCenter,
            ],
            AiPersonality::Balanced => &[
                BuildingRole::Refinery,
                BuildingRole::WarFactory,
                BuildingRole::Barracks,
                BuildingRole::Defense,
                BuildingRole::TechCenter,
            ],
        };
        let missing_tech = profile_build_choice.or_else(|| {
            priority_roles.iter().find_map(|role| {
                if difficulty == AiDifficulty::Easy && *role == BuildingRole::TechCenter {
                    return None;
                }
                let already_owns_role = owned_buildings.iter().any(|ob_id| {
                    loaded
                        .buildings
                        .get(ob_id)
                        .map(|ob| ob.role == *role && !ob.water_bound)
                        .unwrap_or(false)
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

        // On Hard/Brutal or Turtle personality, build extra base defense turrets when wealthy
        let turret_threshold = if personality == AiPersonality::Turtle {
            1400
        } else {
            2400
        };
        let wants_naval_yard = missing_tech.is_none()
            && !world.grid.water_tiles.is_empty()
            && !avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .any(|b| b.water_bound && owned_buildings.contains(&b.id));
        if wants_naval_yard {
            avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .find(|b| b.water_bound)
        } else if missing_tech.is_none()
            && (matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal)
                || personality == AiPersonality::Turtle)
            && econ.credits >= turret_threshold
        {
            avail_buildings
                .iter()
                .filter_map(|b_id| loaded.buildings.get(b_id))
                .find(|b| b.role == BuildingRole::Defense)
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

    if let Some(b_def) = desired_building {
        if econ.credits >= b_def.cost {
            let spot_opt = if b_def.water_bound {
                find_water_build_spot_near_base(world, ai_player_id, b_def.footprint, loaded)
            } else {
                find_build_spot_near_base(world, ai_player_id, b_def.footprint, loaded)
            };
            if let Some(place_tile) = spot_opt {
                commands.push(GameCommand::PlaceBuilding {
                    player_id: ai_player_id,
                    building_id: b_def.id.clone(),
                    tile_pos: place_tile,
                });
                reserved_credits = reserved_credits.max(b_def.cost);
            }
        } else {
            reserved_credits = reserved_credits.max(b_def.cost.min(1600));
        }
    }

    // 2. Check if the AI's base is under attack (Scramble Defense)
    let base_intruder_id = find_enemy_intruder_near_base(world, ai_player_id, base_world, 28.0);
    if base_intruder_id.is_some() {
        reserved_credits = 0;
    }

    let spendable_credits = econ.credits.saturating_sub(reserved_credits);

    // 3. Balanced Unit Production Logic
    let avail_units = world.available_units_for_player(ai_player_id, loaded);
    let has_idle_barracks = world.entities.values().any(|e| {
        e.player_id == ai_player_id
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && e.production_queue.is_empty()
            && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).map(|b| b.role == BuildingRole::Barracks).unwrap_or(false))
    });
    let has_idle_war_factory = world.entities.values().any(|e| {
        e.player_id == ai_player_id
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && e.production_queue.is_empty()
            && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).map(|b| matches!(b.role, BuildingRole::WarFactory | BuildingRole::Airfield)).unwrap_or(false))
    });

    let current_infantry_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).map(|u| u.role == UnitRole::Infantry).unwrap_or(false))
        })
        .count();

    let has_conyard = owned_buildings.iter().any(|ob_id| {
        loaded
            .buildings
            .get(ob_id)
            .map(|b| b.role == BuildingRole::ConstructionYard)
            .unwrap_or(false)
    });
    let owned_mcv_id = world.entities.values().find_map(|e| {
        if e.player_id != ai_player_id || e.health <= 0.0 || e.transported_by.is_some() {
            return None;
        }
        match &e.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .filter(|u| u.transforms_into_building.is_some())
                .map(|_| e.id),
            _ => None,
        }
    });
    if !has_conyard {
        if let Some(mcv_id) = owned_mcv_id {
            commands.push(GameCommand::ActivateAbility {
                player_id: ai_player_id,
                entity_ids: vec![mcv_id],
            });
        }
    }

    let current_harvester_count = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == ai_player_id
                && e.health > 0.0
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).map(|u| u.role == UnitRole::Harvester).unwrap_or(false))
        })
        .count();

    let score_unit_by_profile = |u: &rts_sdk::schema::UnitDef| -> (u64, u32) {
        if let Some(prof) = active_profile {
            if !prof.unit_weights.is_empty() {
                let weight = prof.unit_weights.get(&u.id).copied().unwrap_or(0) as u64;
                if weight > 0 {
                    let owned_cnt = world
                        .entities
                        .values()
                        .filter(|e| {
                            e.player_id == ai_player_id
                                && e.health > 0.0
                                && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == &u.id)
                        })
                        .count() as u64;
                    return ((weight * 1000) / (owned_cnt + 1), u.cost);
                }
            }
        }
        (0, u.cost)
    };

    if has_idle_war_factory {
        let needs_emergency_harvester = current_harvester_count == 0;
        let wants_second_harvester = matches!(difficulty, AiDifficulty::Hard | AiDifficulty::Brutal)
            && current_harvester_count < 2
            && spendable_credits >= 1400;
        let needs_replacement_mcv = !has_conyard && owned_mcv_id.is_none();

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
        } else if needs_replacement_mcv {
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
        } else if let Some(best_vehicle) = avail_units
            .iter()
            .filter_map(|u_id| loaded.units.get(u_id))
            .filter(|u| {
                u.role == UnitRole::Vehicle
                    && u.movement_domain != MovementDomain::Naval
                    && u.transforms_into_building.is_none()
                    && u.primary_weapon.is_some()
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
        e.player_id == 0 && e.health > 0.0 && matches!(e.kind, EntityKind::Building { .. })
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
                && matches!(&e.kind, EntityKind::Unit { unit_id } if loaded.units.get(unit_id).map(|u| u.role == UnitRole::Engineer).unwrap_or(false))
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
        && current_infantry_count < max_infantry
        && (!has_idle_war_factory || difficulty == AiDifficulty::Brutal)
    {
        if let Some(best_infantry) = avail_units
            .iter()
            .filter_map(|u_id| loaded.units.get(u_id))
            .filter(|u| u.role == UnitRole::Infantry && u.cost <= spendable_credits)
            .max_by_key(|u| score_unit_by_profile(u))
        {
            commands.push(GameCommand::ProduceUnit {
                player_id: ai_player_id,
                producer_entity_id: None,
                unit_id: best_infantry.id.clone(),
            });
        }
    }

    // Naval unit & amphibious transport production when AI owns an idle water-bound shipyard
    let has_idle_shipyard = world.entities.values().any(|e| {
        e.player_id == ai_player_id
            && e.health > 0.0
            && e.construction_progress >= 1.0
            && e.production_queue.is_empty()
            && matches!(&e.kind, EntityKind::Building { building_id } if loaded.buildings.get(building_id).map(|b| b.water_bound).unwrap_or(false))
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
                            if loaded.units.get(unit_id).map(|u| u.movement_domain == MovementDomain::Naval).unwrap_or(false)
                    )
            })
            .count();
        let owned_amphibious_transports = world
            .entities
            .values()
            .filter(|e| {
                e.player_id == ai_player_id
                    && e.health > 0.0
                    && matches!(
                        &e.kind,
                        EntityKind::Unit { unit_id }
                            if loaded.units.get(unit_id).map(|u| u.movement_domain == MovementDomain::Amphibious && u.container.accepts_vehicles).unwrap_or(false)
                    )
            })
            .count();
        let wants_hover_transport = owned_warships >= 3 && owned_amphibious_transports == 0;
        let chosen_shipyard_unit = if wants_hover_transport {
            avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .find(|u| {
                    u.movement_domain == MovementDomain::Amphibious
                        && u.container.accepts_vehicles
                        && u.cost <= spendable_credits
                })
        } else {
            None
        }
        .or_else(|| {
            avail_units
                .iter()
                .filter_map(|u_id| loaded.units.get(u_id))
                .filter(|u| u.movement_domain == MovementDomain::Naval && u.cost <= spendable_credits)
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
        personality,
        active_profile.and_then(|p| p.min_attack_wave_size),
        base_world,
        staging_world,
        base_intruder_id,
        &mut commands,
    );
    commands
}
