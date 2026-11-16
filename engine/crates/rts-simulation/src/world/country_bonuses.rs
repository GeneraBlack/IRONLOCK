//! RA2/YR playable country (`SubFactionDef`) passive bonuses & production/spawn hooks.

pub use super::airpad_docking::{resolve_deterministic_sub_faction, splitmix64_country_roll};
use super::{ActiveBuff, EntityKind, PlayerEconomy, SimEntity, SimWorld};
pub(crate) use crate::combat::weather_storm::{
    post_combat_radiation_shields, pre_combat_radiation_shields,
};
use crate::command::GameCommand;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{
    ArmorType, BuildingRole, MovementDomain, ProjectileStyle, SubFactionDef, SupportPowerDef,
    SupportPowerKind, UnitDef, UnitRole,
};
use std::collections::{BTreeMap, BTreeSet};

fn is_psychic_or_cloning_unit(u: &UnitDef, loaded: &LoadedMod) -> bool {
    u.mind_control_limit > 0
        || u.psi_wave_radius > 0.0
        || (u.faction.eq_ignore_ascii_case("yuri")
            && matches!(u.role, UnitRole::Infantry | UnitRole::Engineer))
        || u.primary_weapon
            .as_deref()
            .and_then(|wid| loaded.weapons.get(wid))
            .is_some_and(|w| w.mind_control || w.projectile_style == ProjectileStyle::MindControl)
}

fn normalize_free_power_id(raw: &str, loaded: &LoadedMod) -> String {
    let has = |id: &str| {
        loaded
            .buildings
            .values()
            .any(|b| b.support_powers.iter().any(|sp| sp.id == id))
    };
    if !has(raw) && raw.eq_ignore_ascii_case("airborne_paradrop") && has("allied_airborne_paradrop")
    {
        "allied_airborne_paradrop".to_string()
    } else {
        raw.to_string()
    }
}

fn scale_entity_hp(ent: &mut SimEntity, mult: f32) {
    if (mult - 1.0).abs() > 1e-5 {
        ent.max_health = (ent.max_health * mult).max(1.0);
        ent.health = (ent.health * mult).clamp(1.0, ent.max_health);
        ent.script_ctx.max_health = ent.max_health;
        ent.script_ctx.health = ent.health;
    }
}

fn upsert_passive_rof(ent: &mut SimEntity, id: &str, rof_mult: f32) {
    ent.upsert_status(ActiveBuff {
        id: id.to_string(),
        remaining_secs: 999_999.0,
        fire_rate_mult: rof_mult,
        ..Default::default()
    });
}

fn deduct_credits_exact(econ: Option<&mut PlayerEconomy>, amt: u32) {
    if amt > 0 {
        if let Some(e) = econ {
            let mut m = BTreeMap::new();
            m.insert("credits".to_string(), amt);
            crate::world::economy::deduct_costs(e, &m);
        }
    }
}

impl SimWorld {
    pub fn player_sub_faction_def<'a>(
        &self,
        player_id: u8,
        loaded: &'a LoadedMod,
    ) -> Option<&'a SubFactionDef> {
        let econ = self.players.get(&player_id)?;
        let sub = econ
            .sub_faction
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        let canonical = if sub.eq_ignore_ascii_case("america") {
            "usa"
        } else {
            sub
        };
        loaded
            .factions
            .get(&econ.faction)
            .and_then(|f| {
                f.sub_factions
                    .iter()
                    .find(|sf| sf.id.eq_ignore_ascii_case(canonical))
            })
            .or_else(|| {
                loaded
                    .factions
                    .values()
                    .flat_map(|f| f.sub_factions.iter())
                    .find(|sf| sf.id.eq_ignore_ascii_case(canonical))
            })
    }

    pub fn assign_deterministic_countries_for_slots(
        &mut self,
        ai_player_ids: &[u8],
        seed: u64,
        loaded: &LoadedMod,
    ) {
        let pids: Vec<u8> = self.players.keys().copied().collect();
        for pid in pids {
            let resolved = self.players.get(&pid).and_then(|econ| {
                resolve_deterministic_sub_faction(
                    loaded,
                    &econ.faction,
                    econ.sub_faction.as_deref(),
                    ai_player_ids.contains(&pid),
                    seed,
                    pid,
                )
            });
            if let (Some(sub), Some(econ)) = (resolved, self.players.get_mut(&pid)) {
                econ.sub_faction = Some(sub);
            }
        }
    }

    pub fn country_unit_cost_multiplier(
        &self,
        player_id: u8,
        u_def: &UnitDef,
        loaded: &LoadedMod,
    ) -> f32 {
        let Some(sf) = self.player_sub_faction_def(player_id, loaded) else {
            return 1.0;
        };
        let mut mult = if matches!(
            u_def.movement_domain,
            MovementDomain::Air | MovementDomain::Jumpjet
        ) {
            sf.aircraft_cost_mult
        } else if matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer) {
            sf.infantry_cost_mult
        } else if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester) {
            sf.vehicle_cost_mult
        } else {
            1.0
        };
        if (sf.psychic_cost_mult - 1.0).abs() > 1e-5 && is_psychic_or_cloning_unit(u_def, loaded) {
            mult *= sf.psychic_cost_mult;
        }
        mult.clamp(0.10, 5.0)
    }

    pub fn effective_country_unit_cost(
        &self,
        player_id: u8,
        u_def: &UnitDef,
        loaded: &LoadedMod,
    ) -> u32 {
        let base = self.effective_unit_cost(player_id, u_def, loaded);
        let mult = self.country_unit_cost_multiplier(player_id, u_def, loaded);
        if base == 0 || (mult - 1.0).abs() <= 1e-5 {
            base
        } else {
            ((base as f32) * mult).round().max(1.0) as u32
        }
    }

    pub fn country_mind_control_cooldown_mult(&self, player_id: u8, loaded: &LoadedMod) -> f32 {
        self.player_sub_faction_def(player_id, loaded)
            .map(|sf| sf.mind_control_cooldown_mult.clamp(0.25, 4.0))
            .unwrap_or(1.0)
    }

    pub fn country_anti_armor_damage_multiplier(
        &self,
        attacker_id: u64,
        target_id: u64,
        loaded: &LoadedMod,
    ) -> f32 {
        let Some(att) = self.entities.get(&attacker_id) else {
            return 1.0;
        };
        let Some(target) = self.entities.get(&target_id) else {
            return 1.0;
        };
        let Some(sf) = self.player_sub_faction_def(att.player_id, loaded) else {
            return 1.0;
        };
        if (sf.anti_armor_damage_mult - 1.0).abs() <= 1e-5 {
            return 1.0;
        }
        let is_veh = matches!(&att.kind, EntityKind::Unit { unit_id }
            if loaded.units.get(unit_id).is_some_and(|u| u.role == UnitRole::Vehicle && u.movement_domain != MovementDomain::Air));
        let is_armored = matches!(target.armor, ArmorType::Medium | ArmorType::Heavy)
            || matches!(&target.kind, EntityKind::Unit { unit_id }
                if loaded.units.get(unit_id).is_some_and(|u| matches!(u.role, UnitRole::Vehicle | UnitRole::Harvester)));
        if is_veh && is_armored {
            sf.anti_armor_damage_mult
        } else {
            1.0
        }
    }

    pub(crate) fn apply_country_unit_spawn_bonus(
        &mut self,
        entity_id: u64,
        player_id: u8,
        loaded: &LoadedMod,
    ) {
        let Some(sf) = self.player_sub_faction_def(player_id, loaded).cloned() else {
            return;
        };
        let Some(ent) = self.entities.get_mut(&entity_id) else {
            return;
        };
        let EntityKind::Unit { ref unit_id } = ent.kind else {
            return;
        };
        let Some(u_def) = loaded.units.get(unit_id) else {
            return;
        };
        let is_air = matches!(
            u_def.movement_domain,
            MovementDomain::Air | MovementDomain::Jumpjet
        );
        if !is_air && matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer) {
            scale_entity_hp(ent, sf.infantry_hp_mult);
            ent.script_ctx.damage_multiplier *= sf.infantry_damage_mult;
            if (sf.infantry_range_mult - 1.0).abs() > 1e-5 {
                let rng = u_def
                    .primary_weapon
                    .as_deref()
                    .and_then(|w| loaded.weapons.get(w))
                    .map(|w| w.range)
                    .unwrap_or(10.0);
                ent.script_ctx.range_bonus += rng * (sf.infantry_range_mult - 1.0);
            }
        } else if is_air {
            scale_entity_hp(ent, sf.aircraft_hp_mult);
            ent.script_ctx.damage_multiplier *= sf.aircraft_damage_mult;
        } else if matches!(u_def.role, UnitRole::Vehicle | UnitRole::Harvester) {
            ent.script_ctx.speed_multiplier *= sf.vehicle_speed_mult;
        }
        if (sf.mind_control_cooldown_mult - 1.0).abs() > 1e-5
            && u_def
                .primary_weapon
                .as_deref()
                .and_then(|w| loaded.weapons.get(w))
                .is_some_and(|w| w.mind_control)
        {
            upsert_passive_rof(
                ent,
                "item_passive::country_mc_rof",
                1.0 / sf.mind_control_cooldown_mult.max(0.25),
            );
        }
    }

    pub(crate) fn apply_country_building_spawn_bonus(
        &mut self,
        entity_id: u64,
        player_id: u8,
        loaded: &LoadedMod,
    ) {
        let Some(sf) = self.player_sub_faction_def(player_id, loaded).cloned() else {
            return;
        };
        if let Some(ref raw_sp) = sf.free_support_power {
            let sp_id = normalize_free_power_id(raw_sp, loaded);
            if let Some(econ) = self.players.get_mut(&player_id) {
                econ.support_power_cooldowns.entry(sp_id).or_insert(0.0);
            }
        }
        let Some(ent) = self.entities.get_mut(&entity_id) else {
            return;
        };
        let EntityKind::Building { ref building_id } = ent.kind else {
            return;
        };
        let Some(b_def) = loaded.buildings.get(building_id) else {
            return;
        };
        if b_def.role == BuildingRole::Defense && (sf.defense_rof_mult - 1.0).abs() > 1e-5 {
            upsert_passive_rof(
                ent,
                "item_passive::country_defense_rof",
                sf.defense_rof_mult,
            );
        }
        if (sf.mind_control_cooldown_mult - 1.0).abs() > 1e-5
            && b_def
                .primary_weapon
                .as_deref()
                .and_then(|w| loaded.weapons.get(w))
                .is_some_and(|w| w.mind_control)
        {
            upsert_passive_rof(
                ent,
                "item_passive::country_mc_rof",
                1.0 / sf.mind_control_cooldown_mult.max(0.25),
            );
        }
    }

    pub(crate) fn append_country_free_support_powers(
        &self,
        player_id: u8,
        loaded: &LoadedMod,
        seen: &mut BTreeSet<String>,
        powers: &mut Vec<(SupportPowerDef, f32)>,
    ) {
        let Some(raw_id) = self
            .player_sub_faction_def(player_id, loaded)
            .and_then(|sf| sf.free_support_power.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return;
        };
        let canonical = normalize_free_power_id(raw_id, loaded);
        if seen.contains(&canonical) || seen.contains(raw_id) {
            return;
        }
        let rem = self
            .players
            .get(&player_id)
            .and_then(|e| {
                e.support_power_cooldowns
                    .get(&canonical)
                    .copied()
                    .or_else(|| e.support_power_cooldowns.get(raw_id).copied())
            })
            .unwrap_or(0.0);
        let sp = loaded
            .buildings
            .values()
            .flat_map(|b| b.support_powers.iter())
            .find(|s| s.id == canonical || s.id == raw_id)
            .cloned()
            .unwrap_or_else(|| SupportPowerDef {
                id: canonical,
                name: "Airborne Paradrop".to_string(),
                kind: SupportPowerKind::Paradrop,
                cooldown_secs: 45.0,
                unit_id: self.default_infantry_for_faction("allies", loaded),
                count: 5,
                radius: 4.0,
                ..Default::default()
            });
        seen.insert(sp.id.clone());
        powers.push((sp, rem));
    }

    pub(crate) fn apply_command_with_country_and_ra2_hooks(
        &mut self,
        cmd: GameCommand,
        loaded: &LoadedMod,
    ) {
        if crate::world::timed_bomb::try_intercept_engineer_defuse_command(self, &cmd, loaded)
            || crate::combat::weather_storm::try_intercept_weather_storm_command(self, &cmd, loaded)
        {
            return;
        }
        match cmd {
            GameCommand::ProduceUnit {
                player_id,
                producer_entity_id,
                unit_id,
            } => {
                let Some(u_def) = loaded.units.get(&unit_id) else {
                    return;
                };
                let base = self.effective_unit_cost(player_id, u_def, loaded);
                let disc = self.effective_country_unit_cost(player_id, u_def, loaded);
                if disc != base
                    && self
                        .players
                        .get(&player_id)
                        .is_none_or(|e| e.credits < disc)
                {
                    return;
                }
                let rebate = base.saturating_sub(disc);
                if rebate > 0 {
                    if let Some(econ) = self.players.get_mut(&player_id) {
                        crate::world::economy::add_resource(econ, "credits", rebate, false);
                    }
                }
                let q0 = self.player_unit_count_including_queued(player_id, &unit_id);
                self.apply_command(
                    GameCommand::ProduceUnit {
                        player_id,
                        producer_entity_id,
                        unit_id: unit_id.clone(),
                    },
                    loaded,
                );
                let queued = self.player_unit_count_including_queued(player_id, &unit_id) > q0;
                let claw = if !queued {
                    rebate
                } else {
                    disc.saturating_sub(base)
                };
                deduct_credits_exact(self.players.get_mut(&player_id), claw);
            }
            GameCommand::CancelProduction { player_id, unit_id } => {
                let rebate = loaded
                    .units
                    .get(&unit_id)
                    .map(|u| {
                        self.effective_unit_cost(player_id, u, loaded)
                            .saturating_sub(self.effective_country_unit_cost(player_id, u, loaded))
                    })
                    .unwrap_or(0);
                let q0 = self.player_unit_count_including_queued(player_id, &unit_id);
                self.apply_command(
                    GameCommand::CancelProduction {
                        player_id,
                        unit_id: unit_id.clone(),
                    },
                    loaded,
                );
                if self.player_unit_count_including_queued(player_id, &unit_id) < q0 {
                    deduct_credits_exact(self.players.get_mut(&player_id), rebate);
                }
            }
            GameCommand::CastSupportPower {
                player_id,
                power_id,
                source_pos,
                target_pos,
            } => {
                self.apply_command(
                    GameCommand::CastSupportPower {
                        player_id,
                        power_id: normalize_free_power_id(&power_id, loaded),
                        source_pos,
                        target_pos,
                    },
                    loaded,
                );
            }
            other => self.apply_command(other, loaded),
        }
    }
}
