use crate::ai::{generate_ai_commands_with_difficulty, AiDifficulty};
use crate::command::GameCommand;
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MapDef, MovementDomain, PlayerSpawnDef, UnitRole};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitDuelEntry {
    pub attacker_id: String,
    pub defender_id: String,
    pub attacker_wins: bool,
    pub ticks_to_resolve: u64,
    pub winner_remaining_hp_pct: f32,
    pub cost_efficiency_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostEfficiencyOutlier {
    pub unit_id: String,
    pub unit_name: String,
    pub cost: u32,
    pub avg_cost_efficiency: f32,
    pub win_rate_pct: f32,
    pub warning: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BalanceMatrixReport {
    pub mod_id: String,
    pub duels: Vec<UnitDuelEntry>,
    pub outliers: Vec<CostEfficiencyOutlier>,
}

impl BalanceMatrixReport {
    pub fn to_markdown_table(&self) -> String {
        let mut out = format!("# Balance Duel Matrix & Cost-Efficiency Report (`{}`)\n\n", self.mod_id);
        out.push_str("| Attacker | Defender | Winner | Ticks | Winner HP % | Attacker Cost-Eff |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for d in &self.duels {
            let winner = if d.attacker_wins {
                &d.attacker_id
            } else {
                &d.defender_id
            };
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {:.1}% | {:.2}x |\n",
                d.attacker_id,
                d.defender_id,
                winner,
                d.ticks_to_resolve,
                d.winner_remaining_hp_pct * 100.0,
                d.cost_efficiency_ratio
            ));
        }
        out.push_str("\n## Automated Cost-Efficiency Outlier Warnings (> 2.0x)\n\n");
        if self.outliers.is_empty() {
            out.push_str("_No cost-efficiency outliers (> 2.0x) detected._\n");
        } else {
            for o in &self.outliers {
                out.push_str(&format!(
                    "- **WARNING [`{}` ({}) — ${}]**: Avg Cost-Efficiency `{:.2}x`, Win-Rate `{:.1}%` — {}\n",
                    o.unit_id, o.unit_name, o.cost, o.avg_cost_efficiency, o.win_rate_pct, o.warning
                ));
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionMatchupSummary {
    pub faction_a: String,
    pub faction_b: String,
    pub matches_played: u32,
    pub faction_a_wins: u32,
    pub faction_b_wins: u32,
    pub draws: u32,
    pub avg_match_ticks: u64,
    pub faction_a_avg_army_value: u32,
    pub faction_b_avg_army_value: u32,
}

fn empty_duel_arena_map(loaded: &LoadedMod) -> MapDef {
    let mut factions: Vec<String> = loaded.factions.keys().cloned().collect();
    if factions.is_empty() {
        factions.push("default".to_string());
    }
    let f1 = factions[0].clone();
    let f2 = factions.get(1).cloned().unwrap_or_else(|| f1.clone());
    MapDef {
        id: "duel_arena".to_string(),
        name: "Duel Arena".to_string(),
        width: 32,
        height: 32,
        tile_size: 2.0,
        spawns: vec![
            PlayerSpawnDef {
                player_id: 1,
                team_id: 0,
                faction: f1,
                sub_faction: None,
                base_tile: [4, 4],
                initial_buildings: vec![],
                initial_units: vec![],
            },
            PlayerSpawnDef {
                player_id: 2,
                team_id: 0,
                faction: f2,
                sub_faction: None,
                base_tile: [24, 24],
                initial_buildings: vec![],
                initial_units: vec![],
            },
        ],
        ..Default::default()
    }
}

/// Runs a deterministic 1v1 combat duel matrix across representative ground combat units in `loaded`
/// and automatically flags cost-efficiency outliers (`avg_cost_efficiency > 2.0`).
pub fn run_unit_duel_matrix(loaded: &LoadedMod, max_ticks_per_duel: u64) -> BalanceMatrixReport {
    let arena_map = empty_duel_arena_map(loaded);
    let mut combat_units: Vec<String> = loaded
        .units
        .values()
        .filter(|u| {
            u.primary_weapon.is_some()
                && u.cost > 0
                && u.movement_domain == MovementDomain::Ground
                && matches!(u.role, UnitRole::Infantry | UnitRole::Vehicle)
                && !u.kamikaze
        })
        .map(|u| u.id.clone())
        .collect();
    combat_units.sort();
    // Cap to top 8 combat units sorted by cost so headless matrix runs in < 50ms
    combat_units.sort_by_key(|uid| loaded.units.get(uid).map(|u| u.cost).unwrap_or(0));
    if combat_units.len() > 8 {
        combat_units.truncate(8);
    }

    let mut duels = Vec::new();
    let mut stats_by_unit: BTreeMap<String, (f32, u32, u32)> = BTreeMap::new(); // (sum_eff, wins, total)

    for (i, u_a) in combat_units.iter().enumerate() {
        for u_b in combat_units.iter().skip(i + 1) {
            let mut world = SimWorld::from_mod_map(loaded, &arena_map);
            let Some(id_a) = world.spawn_unit(1, u_a, [12, 16], loaded) else {
                continue;
            };
            let Some(id_b) = world.spawn_unit(2, u_b, [16, 16], loaded) else {
                continue;
            };
            world.apply_command(
                GameCommand::AttackTarget {
                    player_id: 1,
                    attacker_ids: vec![id_a],
                    target_entity_id: id_b,
                },
                loaded,
            );
            world.apply_command(
                GameCommand::AttackTarget {
                    player_id: 2,
                    attacker_ids: vec![id_b],
                    target_entity_id: id_a,
                },
                loaded,
            );

            let max_t = max_ticks_per_duel.max(40);
            while world.tick < max_t {
                world.step(loaded);
                let a_alive = world.entities.get(&id_a).map(|e| e.health > 0.0).unwrap_or(false);
                let b_alive = world.entities.get(&id_b).map(|e| e.health > 0.0).unwrap_or(false);
                if !a_alive || !b_alive {
                    break;
                }
            }

            let hp_a = world
                .entities
                .get(&id_a)
                .map(|e| (e.health / e.max_health.max(1.0)).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            let hp_b = world
                .entities
                .get(&id_b)
                .map(|e| (e.health / e.max_health.max(1.0)).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            let cost_a = loaded.units.get(u_a).map(|u| u.cost).unwrap_or(100).max(1) as f32;
            let cost_b = loaded.units.get(u_b).map(|u| u.cost).unwrap_or(100).max(1) as f32;

            let attacker_wins = hp_a >= hp_b;
            let winner_remaining_hp_pct = if attacker_wins { hp_a } else { hp_b };

            // Cost efficiency for A: value of B destroyed / value of A lost
            let val_b_destroyed = cost_b * (1.0 - hp_b);
            let val_a_lost = (cost_a * (1.0 - hp_a)).max(cost_a * 0.15);
            let eff_a = val_b_destroyed / val_a_lost;

            let val_a_destroyed = cost_a * (1.0 - hp_a);
            let val_b_lost = (cost_b * (1.0 - hp_b)).max(cost_b * 0.15);
            let eff_b = val_a_destroyed / val_b_lost;

            let entry_a = stats_by_unit.entry(u_a.clone()).or_insert((0.0, 0, 0));
            entry_a.0 += eff_a;
            if attacker_wins {
                entry_a.1 += 1;
            }
            entry_a.2 += 1;

            let entry_b = stats_by_unit.entry(u_b.clone()).or_insert((0.0, 0, 0));
            entry_b.0 += eff_b;
            if !attacker_wins {
                entry_b.1 += 1;
            }
            entry_b.2 += 1;

            duels.push(UnitDuelEntry {
                attacker_id: u_a.clone(),
                defender_id: u_b.clone(),
                attacker_wins,
                ticks_to_resolve: world.tick,
                winner_remaining_hp_pct,
                cost_efficiency_ratio: eff_a,
            });
        }
    }

    let mut outliers = Vec::new();
    for (uid, (sum_eff, wins, total)) in stats_by_unit {
        if total == 0 {
            continue;
        }
        let avg_eff = sum_eff / (total as f32);
        let win_rate_pct = (wins as f32) * 100.0 / (total as f32);
        if avg_eff > 2.0 {
            let u_def = loaded.units.get(&uid);
            outliers.push(CostEfficiencyOutlier {
                unit_id: uid.clone(),
                unit_name: u_def.map(|u| u.name.clone()).unwrap_or_else(|| uid.clone()),
                cost: u_def.map(|u| u.cost).unwrap_or(0),
                avg_cost_efficiency: avg_eff,
                win_rate_pct,
                warning: format!(
                    "Unit '{}' trades at {:.2}x cost efficiency (> 2.0x threshold). Consider increasing cost or reducing DPS/HP.",
                    uid, avg_eff
                ),
            });
        }
    }

    BalanceMatrixReport {
        mod_id: loaded.manifest.id.clone(),
        duels,
        outliers,
    }
}

/// Runs a batch of deterministic headless AI vs AI skirmish matches between `faction_a` and `faction_b`.
pub fn run_faction_ai_matchup_batch(
    loaded: &LoadedMod,
    faction_a: &str,
    faction_b: &str,
    matches: u32,
    max_ticks: u64,
) -> FactionMatchupSummary {
    let num_matches = matches.max(1);
    let mut a_wins = 0u32;
    let mut b_wins = 0u32;
    let mut draws = 0u32;
    let mut total_ticks = 0u64;
    let mut sum_val_a = 0u64;
    let mut sum_val_b = 0u64;

    for m_idx in 0..num_matches {
        let mut map = loaded.entry_map.clone();
        if let Some(sp1) = map.spawns.get_mut(0) {
            sp1.faction = faction_a.to_string();
        }
        if let Some(sp2) = map.spawns.get_mut(1) {
            sp2.faction = faction_b.to_string();
        }
        let mut world = SimWorld::from_mod_map(loaded, &map);
        // Vary initial credits slightly per match index for deterministic batch diversity
        if m_idx % 2 == 1 {
            if let Some(p1) = world.players.get_mut(&1) {
                p1.credits += 250;
            }
            if let Some(p2) = world.players.get_mut(&2) {
                p2.credits += 250;
            }
        }

        let limit = max_ticks.max(20);
        while world.tick < limit && world.winner_player_id.is_none() {
            for cmd in generate_ai_commands_with_difficulty(&world, 1, loaded, AiDifficulty::Hard) {
                world.push_command(cmd);
            }
            for cmd in generate_ai_commands_with_difficulty(&world, 2, loaded, AiDifficulty::Hard) {
                world.push_command(cmd);
            }
            world.step(loaded);
        }
        total_ticks += world.tick;

        let army_value = |pid: u8, w: &SimWorld| -> u32 {
            w.entities
                .values()
                .filter(|e| e.player_id == pid && e.health > 0.0)
                .map(|e| match &e.kind {
                    EntityKind::Unit { unit_id } => {
                        loaded.units.get(unit_id).map(|u| u.cost).unwrap_or(100)
                    }
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .map(|b| b.cost)
                        .unwrap_or(200),
                })
                .sum()
        };
        let val_a = army_value(1, &world);
        let val_b = army_value(2, &world);
        sum_val_a += val_a as u64;
        sum_val_b += val_b as u64;

        match world.winner_player_id {
            Some(1) => a_wins += 1,
            Some(2) => b_wins += 1,
            _ => {
                if val_a > val_b + 300 {
                    a_wins += 1;
                } else if val_b > val_a + 300 {
                    b_wins += 1;
                } else {
                    draws += 1;
                }
            }
        }
    }

    FactionMatchupSummary {
        faction_a: faction_a.to_string(),
        faction_b: faction_b.to_string(),
        matches_played: num_matches,
        faction_a_wins: a_wins,
        faction_b_wins: b_wins,
        draws,
        avg_match_ticks: total_ticks / (num_matches as u64),
        faction_a_avg_army_value: (sum_val_a / (num_matches as u64)) as u32,
        faction_b_avg_army_value: (sum_val_b / (num_matches as u64)) as u32,
    }
}
