//! Data-driven hero progression (`1..N` XP levels, radius XP sharing, skill ranks `1..=4`,
//! altar/HQ revival), `N`-slot relic/item inventory, independent per-ability cooldowns + energy +
//! auto-cast, and `ScriptContext` 2.0 action execution (`Task 232`).

use crate::combat::SimProjectile;
use crate::command::{GameCommand, UnitStance};
use crate::det_math::{det_distance_xz, SimScalar};
use crate::world::economy::{can_afford_costs, deduct_costs, sync_player_ledgers};
use crate::world::{
    ActiveBuff, EntityKind, FallenHeroRecord, HeroInstanceState, HeroRevivalJob, InventorySlotItem,
    SimEntity, SimGroundItem, SimWorld, TRAIT_ENERGY, TRAIT_HERO,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::manifest::EconomyModel;
use rts_sdk::schema::{
    AbilityDef, AbilityTargetMode, BuildingRole, DeliveryKind, EffectPayload, MovementDomain,
    StackPolicy, StatusEffectDef, UnitDef,
};
use rts_sdk::scripting::ScriptAction;
use std::collections::BTreeMap;

impl SimWorld {
    /// Returns the effective rank (`0` = unlearned hero skill, `1..=max_rank` = active rank) for `ab`.
    pub fn effective_ability_rank(
        &self,
        ent_id: u64,
        unit_id_opt: Option<&str>,
        ab: &AbilityDef,
        loaded: &LoadedMod,
    ) -> u8 {
        let learned_rank = self
            .trait_stores
            .heroes
            .get(&ent_id)
            .and_then(|h| h.learned_skills.get(&ab.id).copied());
        if let Some(uid) = unit_id_opt {
            if let Some(hero_spec) = loaded.units.get(uid).and_then(|u| u.hero_trait.as_ref()) {
                if hero_spec.learnable_abilities.iter().any(|id| id == &ab.id) {
                    return learned_rank.unwrap_or(0);
                }
            }
        }
        learned_rank.unwrap_or(1).max(1)
    }

    #[inline]
    pub fn scaled_ability_cooldown(ab: &AbilityDef, rank: u8) -> f32 {
        if ab.cooldown_secs <= 0.0 {
            return 0.0;
        }
        let extra = rank.saturating_sub(1) as f32;
        (ab.cooldown_secs - ab.cooldown_reduction_per_rank * extra).max(0.25)
    }

    #[inline]
    pub fn scaled_ability_value_mult(ab: &AbilityDef, rank: u8) -> f32 {
        let extra = rank.saturating_sub(1) as f32;
        (1.0 + ab.value_scale_per_rank * extra).max(0.1)
    }

    pub fn scale_effect_payload(payload: &EffectPayload, value_mult: f32) -> EffectPayload {
        if (value_mult - 1.0).abs() <= 1e-4 {
            return payload.clone();
        }
        match payload {
            EffectPayload::AreaDamage {
                radius,
                damage,
                friendly_fire,
            } => EffectPayload::AreaDamage {
                radius: *radius,
                damage: damage * value_mult,
                friendly_fire: *friendly_fire,
            },
            EffectPayload::SpawnHazardZone {
                radius,
                dps,
                duration_secs,
                is_toxic_cloud,
            } => EffectPayload::SpawnHazardZone {
                radius: *radius,
                dps: dps * value_mult,
                duration_secs: *duration_secs,
                is_toxic_cloud: *is_toxic_cloud,
            },
            EffectPayload::ApplyAreaStatus {
                radius,
                status,
                kills_organic_infantry,
                buildings_only,
                friendly_only,
            } => EffectPayload::ApplyAreaStatus {
                radius: *radius,
                status: Self::scale_status_def(status, value_mult),
                kills_organic_infantry: *kills_organic_infantry,
                buildings_only: *buildings_only,
                friendly_only: *friendly_only,
            },
            other => other.clone(),
        }
    }

    pub fn scale_status_def(status: &StatusEffectDef, value_mult: f32) -> StatusEffectDef {
        if (value_mult - 1.0).abs() <= 1e-4 {
            return status.clone();
        }
        let mut s = status.clone();
        if s.dot_dps > 0.0 {
            s.dot_dps *= value_mult;
        }
        if s.hp_regen_per_sec > 0.0 {
            s.hp_regen_per_sec *= value_mult;
        }
        if let Some(ref mut m) = s.modifier {
            if m.hp_regen_per_sec > 0.0 {
                m.hp_regen_per_sec *= value_mult;
            }
        }
        s
    }

    /// Deducts `energy_cost` from an entity's `EnergyPoolState` if sufficient energy is available.
    pub fn try_consume_entity_energy(&mut self, ent_id: u64, energy_cost: f32) -> bool {
        if energy_cost <= 0.0 {
            return true;
        }
        let cur = self
            .trait_stores
            .energy_pools
            .get(&ent_id)
            .map(|ep| ep.energy)
            .or_else(|| self.entities.get(&ent_id).map(|e| e.energy))
            .unwrap_or(0.0);
        if cur + 1e-4 < energy_cost {
            return false;
        }
        let next_e = (cur - energy_cost).max(0.0);
        if let Some(ep) = self.trait_stores.energy_pools.get_mut(&ent_id) {
            ep.energy = next_e;
        }
        if let Some(ent) = self.entities.get_mut(&ent_id) {
            ent.energy = next_e;
            ent.script_ctx.energy = next_e;
        }
        true
    }

    /// Recomputes a hero's `max_health`, `max_energy`, level damage scaling buff, and equipped item passive buffs.
    pub fn recompute_hero_stats_and_item_buffs(&mut self, hero_id: u64, loaded: &LoadedMod) {
        let Some((unit_id, player_id)) = self.entities.get(&hero_id).and_then(|e| match &e.kind {
            EntityKind::Unit { unit_id } => Some((unit_id.clone(), e.player_id)),
            _ => None,
        }) else {
            return;
        };
        let Some(u_def) = loaded.units.get(&unit_id) else {
            return;
        };
        let Some(hero_spec) = u_def.hero_trait.as_ref() else {
            return;
        };
        let Some(hero_state) = self.trait_stores.heroes.get(&hero_id).cloned() else {
            return;
        };

        let lvl_extra = hero_state.level.saturating_sub(1) as f32;
        let handicap_mult = self
            .players
            .get(&player_id)
            .map(|p| {
                let pct = if p.handicap_pct == 0 {
                    100
                } else {
                    p.handicap_pct
                };
                (pct as f32) / 100.0
            })
            .unwrap_or(1.0);

        let mut bonus_hp = hero_spec.hp_per_level * lvl_extra;
        let mut bonus_energy = hero_spec.energy_per_level * lvl_extra;
        let mut item_buffs: Vec<ActiveBuff> = Vec::new();

        // Count duplicate items for StackIntensity support across inventory slots
        let mut item_counts: BTreeMap<String, u8> = BTreeMap::new();
        for slot_item in hero_state.inventory.iter().flatten() {
            *item_counts.entry(slot_item.item_id.clone()).or_insert(0) += 1;
            if let Some(item_def) = loaded.items.get(&slot_item.item_id) {
                bonus_hp += item_def.bonus_max_health;
                bonus_energy += item_def.bonus_max_energy;
            }
        }

        for (item_id, count) in item_counts {
            let Some(item_def) = loaded.items.get(&item_id) else {
                continue;
            };
            let pm = &item_def.passive_modifiers;
            let has_passive = (pm.speed_mult - 1.0).abs() > 1e-5
                || (pm.damage_mult - 1.0).abs() > 1e-5
                || (pm.armor_mult - 1.0).abs() > 1e-5
                || pm.range_bonus.abs() > 1e-5
                || (pm.fire_rate_mult - 1.0).abs() > 1e-5
                || (pm.vision_mult - 1.0).abs() > 1e-5
                || (pm.build_speed_mult - 1.0).abs() > 1e-5
                || pm.hp_regen_per_sec > 0.0;
            if !has_passive {
                continue;
            }
            let stacks = if pm.stack_policy == StackPolicy::Refresh {
                1
            } else {
                count.min(pm.max_stacks.max(1))
            };
            item_buffs.push(ActiveBuff {
                id: format!("item_passive::{item_id}"),
                remaining_secs: 999_999.0,
                speed_multiplier: pm.speed_mult,
                damage_multiplier: pm.damage_mult,
                armor_multiplier: pm.armor_mult,
                range_bonus: pm.range_bonus,
                fire_rate_mult: pm.fire_rate_mult,
                vision_mult: pm.vision_mult,
                build_speed_mult: pm.build_speed_mult,
                hp_regen_per_sec: pm.hp_regen_per_sec,
                stack_policy: pm.stack_policy,
                max_stacks: pm.max_stacks.max(1),
                stacks: stacks.max(1),
                dispel_category: None,
                source_entity_id: Some(hero_id),
                attacker_player_id: player_id,
                ..Default::default()
            });
        }

        if lvl_extra > 0.0 && hero_spec.damage_mult_per_level.abs() > 1e-5 {
            item_buffs.push(ActiveBuff {
                id: "hero_level_scaling".to_string(),
                remaining_secs: 999_999.0,
                damage_multiplier: (1.0 + hero_spec.damage_mult_per_level * lvl_extra).max(0.1),
                source_entity_id: Some(hero_id),
                attacker_player_id: player_id,
                ..Default::default()
            });
        }

        let new_max_hp = (u_def.health * handicap_mult + bonus_hp).max(1.0);
        let base_max_energy = u_def
            .energy_pool
            .as_ref()
            .map(|ep| ep.max_energy)
            .unwrap_or(0.0);
        let new_max_energy = if u_def.energy_pool.is_some() || bonus_energy > 0.0 {
            (base_max_energy + bonus_energy).max(0.0)
        } else {
            0.0
        };

        if let Some(ent) = self.entities.get_mut(&hero_id) {
            let hp_delta = new_max_hp - ent.max_health;
            ent.max_health = new_max_hp;
            if hp_delta > 0.0 && ent.health > 0.0 {
                ent.health = (ent.health + hp_delta).min(ent.max_health);
            } else {
                ent.health = ent.health.min(ent.max_health);
            }
            ent.script_ctx.max_health = ent.max_health;
            ent.script_ctx.health = ent.health;

            let energy_delta = new_max_energy - ent.max_energy;
            ent.max_energy = new_max_energy;
            if new_max_energy > 0.0 {
                ent.trait_flags |= TRAIT_ENERGY;
                let base_regen = u_def
                    .energy_pool
                    .as_ref()
                    .map(|ep| ep.regen_per_sec)
                    .unwrap_or(1.0);
                let ep_entry = self.trait_stores.energy_pools.entry(hero_id).or_insert(
                    crate::world::EnergyPoolState {
                        energy: ent.energy,
                        max_energy: new_max_energy,
                        regen_per_sec: base_regen,
                    },
                );
                ep_entry.max_energy = new_max_energy;
                if energy_delta > 0.0 {
                    ep_entry.energy = (ep_entry.energy + energy_delta).min(new_max_energy);
                } else {
                    ep_entry.energy = ep_entry.energy.min(new_max_energy);
                }
                ent.energy = ep_entry.energy;
            }
            ent.script_ctx.max_energy = ent.max_energy;
            ent.script_ctx.energy = ent.energy;

            ent.active_buffs
                .retain(|b| !b.id.starts_with("item_passive::") && b.id != "hero_level_scaling");
            ent.active_buffs.extend(item_buffs);
        }
    }

    /// Distributes XP to allied heroes within `xp_share_radius` when an enemy unit/building is killed.
    pub fn award_hero_experience_on_kill(
        &mut self,
        killer_player_id: u8,
        victim_pos: [f32; 3],
        victim_def_id: &str,
        victim_is_building: bool,
        loaded: &LoadedMod,
    ) {
        if killer_player_id == 0 || self.trait_stores.heroes.is_empty() {
            return;
        }
        let base_xp = if victim_is_building {
            loaded
                .buildings
                .get(victim_def_id)
                .map(|b| (b.cost.max(100) as f32) * 0.45 + b.health * 0.05)
                .unwrap_or(80.0)
        } else {
            loaded
                .units
                .get(victim_def_id)
                .map(|u| (u.cost.max(50) as f32) * 0.60 + u.health * 0.08)
                .unwrap_or(60.0)
        };
        if base_xp <= 0.0 {
            return;
        }

        let mut recipients: Vec<(u64, String)> = Vec::new();
        for &hid in self.trait_stores.heroes.keys() {
            let Some(ent) = self.entities.get(&hid) else {
                continue;
            };
            if ent.health <= 0.0 || self.are_hostile(killer_player_id, ent.player_id) {
                continue;
            }
            let EntityKind::Unit { ref unit_id } = ent.kind else {
                continue;
            };
            let share_r = loaded
                .units
                .get(unit_id)
                .and_then(|u| u.hero_trait.as_ref())
                .map(|h| h.xp_share_radius.max(8.0))
                .unwrap_or(18.0);
            if det_distance_xz(ent.position, victim_pos) <= SimScalar::from_f32(share_r) {
                recipients.push((hid, unit_id.clone()));
            }
        }
        if recipients.is_empty() {
            return;
        }

        let share_xp =
            (SimScalar::from_f32(base_xp) / SimScalar::from_num(recipients.len() as i32)).to_f32();
        let mut leveled_up_ids: Vec<u64> = Vec::new();
        for (hid, unit_id) in recipients {
            let Some(hero_spec) = loaded
                .units
                .get(&unit_id)
                .and_then(|u| u.hero_trait.as_ref())
            else {
                continue;
            };
            let new_xp = if let Some(ent) = self.entities.get_mut(&hid) {
                ent.experience += share_xp;
                ent.experience
            } else {
                continue;
            };
            let max_lvl = hero_spec.max_level.max(1);
            let mut did_level = false;
            if let Some(h_state) = self.trait_stores.heroes.get_mut(&hid) {
                while h_state.level < max_lvl {
                    let Some(req_xp) = hero_spec.xp_threshold_for_next_level(h_state.level) else {
                        break;
                    };
                    if new_xp + 1e-4 >= req_xp {
                        h_state.level += 1;
                        h_state.skill_points = h_state.skill_points.saturating_add(1);
                        did_level = true;
                    } else {
                        break;
                    }
                }
            }
            if did_level {
                leveled_up_ids.push(hid);
            }
        }
        for hid in leveled_up_ids {
            self.recompute_hero_stats_and_item_buffs(hid, loaded);
        }
    }

    /// Captures a dying hero into `trait_stores.fallen_heroes[player_id]` so it can be revived at an altar/HQ.
    pub fn capture_fallen_hero_on_death(&mut self, dead_ent: &SimEntity) {
        if (dead_ent.trait_flags & TRAIT_HERO) == 0 || dead_ent.player_id == 0 {
            return;
        }
        let EntityKind::Unit { ref unit_id } = dead_ent.kind else {
            return;
        };
        let Some(hero_state) = self.trait_stores.heroes.get(&dead_ent.id).cloned() else {
            return;
        };
        let list = self
            .trait_stores
            .fallen_heroes
            .entry(dead_ent.player_id)
            .or_default();
        list.retain(|r| r.unit_id != *unit_id);
        list.push(FallenHeroRecord {
            original_entity_id: dead_ent.id,
            unit_id: unit_id.clone(),
            experience: dead_ent.experience,
            hero_state,
        });
    }

    /// Returns any in-progress hero revival jobs back to `fallen_heroes` if the reviving altar is destroyed.
    pub fn return_unfinished_revivals_from_building(&mut self, building_id: u64, player_id: u8) {
        let Some(jobs) = self.trait_stores.revival_queues.remove(&building_id) else {
            return;
        };
        if player_id == 0 {
            return;
        }
        let fallen = self
            .trait_stores
            .fallen_heroes
            .entry(player_id)
            .or_default();
        for job in jobs {
            if !fallen.iter().any(|r| r.unit_id == job.record.unit_id) {
                fallen.push(job.record);
            }
        }
    }

    pub fn revival_costs_for_hero(u_def: &UnitDef, revive_cost_mult: f32) -> BTreeMap<String, u32> {
        let mult = revive_cost_mult.max(0.0);
        let mut costs = BTreeMap::new();
        for (res_id, &amt) in &u_def.resource_costs {
            let scaled = ((amt as f32) * mult).round() as u32;
            if scaled > 0 {
                costs.insert(res_id.clone(), scaled);
            }
        }
        if !costs.contains_key("credits") && u_def.cost > 0 {
            let scaled = ((u_def.cost as f32) * mult).round() as u32;
            if scaled > 0 {
                costs.insert("credits".to_string(), scaled);
            }
        }
        costs
    }

    fn building_can_revive_hero(
        b_def: &rts_sdk::schema::BuildingDef,
        hero_spec: &rts_sdk::schema::HeroTraitSpec,
    ) -> bool {
        if hero_spec.revive_at_building_tags.is_empty() {
            return b_def.role == BuildingRole::ConstructionYard;
        }
        for req_tag in &hero_spec.revive_at_building_tags {
            if b_def.tags.iter().any(|t| t == req_tag)
                || b_def.id == *req_tag
                || (req_tag == "construction_yard" && b_def.role == BuildingRole::ConstructionYard)
            {
                return true;
            }
        }
        false
    }

    /// Executes a `ToggleAbilityAutoCast`, `LearnHeroSkill`, `ReviveHero`, `PickupItem`,
    /// `UseInventoryItem`, or `DropInventoryItem` command (`Task 232`).
    pub fn apply_hero_and_item_command(&mut self, cmd: GameCommand, loaded: &LoadedMod) -> bool {
        match cmd {
            GameCommand::ToggleAbilityAutoCast {
                player_id,
                entity_ids,
                ability_index,
                enabled,
            } => {
                for eid in entity_ids {
                    let allow = self.entities.get(&eid).is_some_and(|e| {
                        if e.player_id != player_id || e.health <= 0.0 {
                            return false;
                        }
                        match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .and_then(|u| u.abilities.get(ability_index))
                                .is_some_and(|ab| ab.allow_auto_cast),
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .and_then(|b| b.abilities.get(ability_index))
                                .is_some_and(|ab| ab.allow_auto_cast),
                        }
                    });
                    if !allow {
                        continue;
                    }
                    if let Some(ent) = self.entities.get_mut(&eid) {
                        let next_on = enabled
                            .unwrap_or_else(|| !ent.auto_cast_abilities.contains(&ability_index));
                        if next_on {
                            ent.auto_cast_abilities.insert(ability_index);
                        } else {
                            ent.auto_cast_abilities.remove(&ability_index);
                        }
                    }
                }
                true
            }
            GameCommand::LearnHeroSkill {
                player_id,
                hero_id,
                ability_id,
            } => {
                let Some(unit_id) = self.entities.get(&hero_id).and_then(|e| {
                    if e.player_id != player_id || e.health <= 0.0 {
                        return None;
                    }
                    match &e.kind {
                        EntityKind::Unit { unit_id } => Some(unit_id.clone()),
                        _ => None,
                    }
                }) else {
                    return true;
                };
                let Some(u_def) = loaded.units.get(&unit_id) else {
                    return true;
                };
                let Some(hero_spec) = u_def.hero_trait.as_ref() else {
                    return true;
                };
                if !hero_spec.learnable_abilities.is_empty()
                    && !hero_spec
                        .learnable_abilities
                        .iter()
                        .any(|id| id == &ability_id)
                {
                    return true;
                }
                let Some(ab_def) = u_def.abilities.iter().find(|a| a.id == ability_id) else {
                    return true;
                };
                let max_rank = ab_def.max_rank.clamp(1, 4);
                if let Some(h_state) = self.trait_stores.heroes.get_mut(&hero_id) {
                    if h_state.skill_points == 0 {
                        return true;
                    }
                    let cur_rank = h_state
                        .learned_skills
                        .get(&ability_id)
                        .copied()
                        .unwrap_or(0);
                    if cur_rank < max_rank {
                        h_state.learned_skills.insert(ability_id, cur_rank + 1);
                        h_state.skill_points -= 1;
                    }
                }
                true
            }
            GameCommand::ReviveHero {
                player_id,
                altar_entity_id,
                hero_def_id,
            } => {
                sync_player_ledgers(self);
                let Some(resolved_hero_uid) = hero_def_id.or_else(|| {
                    self.trait_stores
                        .fallen_heroes
                        .get(&player_id)
                        .and_then(|list| list.first())
                        .map(|r| r.unit_id.clone())
                }) else {
                    return true;
                };
                let Some(u_def) = loaded.units.get(&resolved_hero_uid) else {
                    return true;
                };
                let Some(hero_spec) = u_def.hero_trait.as_ref() else {
                    return true;
                };
                let resolved_altar_opt = if let Some(aid) = altar_entity_id {
                    self.entities.get(&aid).and_then(|e| {
                        if e.player_id != player_id
                            || e.health <= 0.0
                            || e.construction_progress < 1.0
                            || e.power_disabled
                        {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .filter(|b_def| Self::building_can_revive_hero(b_def, hero_spec))
                                .map(|_| aid),
                            _ => None,
                        }
                    })
                } else {
                    self.entities.values().find_map(|e| {
                        if e.player_id != player_id
                            || e.health <= 0.0
                            || e.construction_progress < 1.0
                            || e.power_disabled
                        {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .filter(|b_def| Self::building_can_revive_hero(b_def, hero_spec))
                                .map(|_| e.id),
                            _ => None,
                        }
                    })
                };
                let Some(chosen_altar_id) = resolved_altar_opt else {
                    return true;
                };
                let already_alive_or_queued = self.entities.values().any(|e| {
                    e.player_id == player_id
                        && e.health > 0.0
                        && matches!(&e.kind, EntityKind::Unit { unit_id } if unit_id == &resolved_hero_uid)
                }) || self.trait_stores.revival_queues.iter().any(|(&bid, q)| {
                    self.entities.get(&bid).is_some_and(|b| b.player_id == player_id)
                        && q.iter().any(|j| j.record.unit_id == resolved_hero_uid)
                });
                if already_alive_or_queued {
                    return true;
                }
                let Some(rec_idx) = self
                    .trait_stores
                    .fallen_heroes
                    .get(&player_id)
                    .and_then(|list| list.iter().position(|r| r.unit_id == resolved_hero_uid))
                else {
                    return true;
                };

                let is_streaming = loaded.manifest.rules.economy_model == EconomyModel::Streaming;
                let costs = Self::revival_costs_for_hero(u_def, hero_spec.revive_cost_mult);
                if !is_streaming {
                    let paid = if let Some(econ) = self.players.get_mut(&player_id) {
                        if can_afford_costs(econ, &costs) {
                            deduct_costs(econ, &costs);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if !paid {
                        return true;
                    }
                }

                let record = self
                    .trait_stores
                    .fallen_heroes
                    .get_mut(&player_id)
                    .unwrap()
                    .remove(rec_idx);
                let total_secs = (hero_spec.revive_time_per_level_secs.max(1.0)
                    * (record.hero_state.level.max(1) as f32))
                    .max(1.0);
                self.trait_stores
                    .revival_queues
                    .entry(chosen_altar_id)
                    .or_default()
                    .push_back(HeroRevivalJob {
                        record,
                        remaining_secs: total_secs,
                        total_secs,
                        paid_fraction: if is_streaming && !costs.is_empty() {
                            0.0
                        } else {
                            1.0
                        },
                    });
                true
            }
            GameCommand::PickupItem {
                player_id,
                hero_id,
                ground_item_id,
            } => {
                let Some((hero_pos, hero_tile)) = self.entities.get(&hero_id).and_then(|e| {
                    (e.player_id == player_id
                        && (e.trait_flags & TRAIT_HERO) != 0
                        && self.can_entity_act(e, loaded))
                    .then_some((e.position, e.tile_origin))
                }) else {
                    return true;
                };
                let Some(gi_idx) = self
                    .ground_items
                    .iter()
                    .position(|g| g.id == ground_item_id)
                else {
                    return true;
                };
                let gi_pos = self.ground_items[gi_idx].position;
                let gi_tile = self.ground_items[gi_idx].tile_pos;
                if det_distance_xz(hero_pos, gi_pos) <= SimScalar::from_f32(5.0) {
                    let _ = self.try_equip_ground_item_index(hero_id, gi_idx, loaded);
                } else {
                    let allowed_gates = self.allowed_gate_owners(player_id);
                    let path = self.grid.find_path_for_player(
                        hero_tile,
                        gi_tile,
                        MovementDomain::Ground,
                        0.0,
                        &allowed_gates,
                    );
                    if let Some(ent) = self.entities.get_mut(&hero_id) {
                        ent.clear_flow_steering();
                        ent.target_entity_id = None;
                        ent.attack_ground_pos = None;
                        ent.path_waypoints = path;
                    }
                }
                true
            }
            GameCommand::UseInventoryItem {
                player_id,
                hero_id,
                slot_index,
                target_pos,
                target_entity_id,
            } => {
                let Some(caster_pos) = self.entities.get(&hero_id).and_then(|e| {
                    (e.player_id == player_id
                        && (e.trait_flags & TRAIT_HERO) != 0
                        && self.can_entity_act(e, loaded))
                    .then_some(e.position)
                }) else {
                    return true;
                };
                let Some(slot_item) = self
                    .trait_stores
                    .heroes
                    .get(&hero_id)
                    .and_then(|h| h.inventory.get(slot_index))
                    .and_then(|s| s.clone())
                else {
                    return true;
                };
                let Some(item_def) = loaded.items.get(&slot_item.item_id).cloned() else {
                    return true;
                };
                if let Some(ref ab) = item_def.active_ability {
                    let eff_pos = target_pos.unwrap_or(caster_pos);
                    if !self.execute_unit_or_item_ability(
                        player_id,
                        hero_id,
                        None,
                        ab,
                        1,
                        eff_pos,
                        target_entity_id,
                        loaded,
                    ) {
                        return true;
                    }
                } else if !item_def.consumable {
                    return true;
                }
                if item_def.consumable {
                    let mut removed_item = false;
                    if let Some(h_state) = self.trait_stores.heroes.get_mut(&hero_id) {
                        if let Some(slot_ref) = h_state.inventory.get_mut(slot_index) {
                            if let Some(ref mut si) = slot_ref {
                                si.charges = si.charges.saturating_sub(1);
                                if si.charges == 0 {
                                    *slot_ref = None;
                                    removed_item = true;
                                }
                            }
                        }
                    }
                    if removed_item {
                        self.recompute_hero_stats_and_item_buffs(hero_id, loaded);
                    }
                }
                true
            }
            GameCommand::DropInventoryItem {
                player_id,
                hero_id,
                slot_index,
                drop_pos,
            } => {
                let Some(hero_pos) = self.entities.get(&hero_id).and_then(|e| {
                    (e.player_id == player_id
                        && (e.trait_flags & TRAIT_HERO) != 0
                        && e.health > 0.0)
                        .then_some(e.position)
                }) else {
                    return true;
                };
                let dropped = self
                    .trait_stores
                    .heroes
                    .get_mut(&hero_id)
                    .and_then(|h| h.inventory.get_mut(slot_index))
                    .and_then(|slot| slot.take());
                if let Some(slot_item) = dropped {
                    let gid = self.next_ground_item_id;
                    self.next_ground_item_id += 1;
                    let target_w = drop_pos.unwrap_or(hero_pos);
                    let drop_tile = self
                        .grid
                        .find_nearest_walkable(self.grid.world_to_tile(target_w));
                    self.ground_items.push(SimGroundItem {
                        id: gid,
                        item_id: slot_item.item_id,
                        tile_pos: drop_tile,
                        position: target_w,
                        remaining_charges: Some(slot_item.charges as u32),
                    });
                    self.recompute_hero_stats_and_item_buffs(hero_id, loaded);
                }
                true
            }
            _ => false,
        }
    }

    pub(crate) fn try_equip_ground_item_index(
        &mut self,
        hero_id: u64,
        ground_item_idx: usize,
        loaded: &LoadedMod,
    ) -> bool {
        if ground_item_idx >= self.ground_items.len() {
            return false;
        }
        let item_id = self.ground_items[ground_item_idx].item_id.clone();
        let default_charges = loaded
            .items
            .get(&item_id)
            .map(|d| d.max_charges.max(1) as u32)
            .unwrap_or(1);
        let charges = self.ground_items[ground_item_idx]
            .remaining_charges
            .unwrap_or(default_charges)
            .clamp(1, 255) as u8;

        let equipped = if let Some(h_state) = self.trait_stores.heroes.get_mut(&hero_id) {
            if let Some(empty_slot) = h_state.inventory.iter_mut().find(|s| s.is_none()) {
                *empty_slot = Some(InventorySlotItem { item_id, charges });
                true
            } else {
                false
            }
        } else {
            false
        };
        if equipped {
            self.ground_items.remove(ground_item_idx);
            self.recompute_hero_stats_and_item_buffs(hero_id, loaded);
        }
        equipped
    }

    /// Unified execution of a unit/building/item `AbilityDef` with independent cooldown index,
    /// `EnergyPoolSpec` cost, rank value/cooldown scaling, and `ScriptContext` 2.0 hook + actions.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_unit_or_item_ability(
        &mut self,
        player_id: u8,
        caster_id: u64,
        ability_slot_index: Option<usize>,
        ab: &AbilityDef,
        rank: u8,
        target_pos: [f32; 3],
        target_entity_id: Option<u64>,
        loaded: &LoadedMod,
    ) -> bool {
        let Some(caster_snap) = self.entities.get(&caster_id).cloned() else {
            return false;
        };
        if let Some(idx) = ability_slot_index {
            if caster_snap.ability_cooldown_at(idx) > 0.0 {
                return false;
            }
        }
        let (eff_target_pos, eff_target_id) = match ab.target_mode {
            AbilityTargetMode::SelfOnly => (caster_snap.position, Some(caster_id)),
            AbilityTargetMode::SingleEntity => {
                let Some(tid) = target_entity_id else {
                    return false;
                };
                let Some(t_pos) = self
                    .entities
                    .get(&tid)
                    .filter(|e| e.is_targetable())
                    .map(|e| e.position)
                else {
                    return false;
                };
                (t_pos, Some(tid))
            }
            AbilityTargetMode::GroundPoint | AbilityTargetMode::AreaRadius => {
                let pos = target_entity_id
                    .and_then(|tid| {
                        self.entities
                            .get(&tid)
                            .filter(|e| e.is_targetable())
                            .map(|e| e.position)
                    })
                    .unwrap_or(target_pos);
                (pos, target_entity_id)
            }
        };

        if ab.range > 0.0 && ab.target_mode != AbilityTargetMode::SelfOnly {
            let max_r = ab.range + 0.5;
            if det_distance_xz(caster_snap.position, eff_target_pos) > SimScalar::from_f32(max_r) {
                return false;
            }
        }

        if ab.cost > 0 {
            let can_afford = self
                .players
                .get(&player_id)
                .is_some_and(|e| e.credits >= ab.cost);
            if !can_afford {
                return false;
            }
        }
        if !self.try_consume_entity_energy(caster_id, ab.energy_cost) {
            return false;
        }
        if ab.cost > 0 {
            if let Some(econ) = self.players.get_mut(&player_id) {
                econ.credits -= ab.cost;
                econ.resources.insert("credits".to_string(), econ.credits);
            }
        }

        let value_mult = Self::scaled_ability_value_mult(ab, rank);
        for payload in &ab.payloads {
            let scaled_payload = Self::scale_effect_payload(payload, value_mult);
            self.execute_effect_payload(
                player_id,
                Some(caster_id),
                Some(caster_snap.position),
                eff_target_pos,
                &scaled_payload,
                loaded,
            );
        }

        if let Some(se) = &ab.applies_status {
            let scaled_se = Self::scale_status_def(se, value_mult);
            let mut buff = ActiveBuff::from_def(&scaled_se);
            buff.attacker_player_id = player_id;
            buff.source_entity_id = Some(caster_id);
            let plants_c4 = buff.delayed_burst_damage > 0.0 || buff.id == "c4_bomb";
            if let Some(tid) = eff_target_id {
                if let Some(t_ent) = self.entities.get_mut(&tid) {
                    if plants_c4 {
                        self.trait_stores
                            .set_planted_c4_owner_on(t_ent, Some(player_id));
                    }
                    t_ent.apply_buff_stack(buff);
                }
            } else if ab.radius > 0.1 {
                let r_sq = ab.radius * ab.radius;
                for e in self.entities.values_mut() {
                    let dx = e.position[0] - eff_target_pos[0];
                    let dz = e.position[2] - eff_target_pos[2];
                    if dx * dx + dz * dz <= r_sq {
                        if plants_c4 {
                            self.trait_stores
                                .set_planted_c4_owner_on(e, Some(player_id));
                        }
                        e.apply_buff_stack(buff.clone());
                    }
                }
            }
        }

        let scaled_cd = Self::scaled_ability_cooldown(ab, rank);
        let mut emitted = Vec::new();
        if let Some(c_mut) = self.entities.get_mut(&caster_id) {
            if scaled_cd > 0.0 {
                if let Some(idx) = ability_slot_index {
                    c_mut.set_ability_cooldown_at(idx, scaled_cd);
                } else {
                    c_mut.ability_cooldown = scaled_cd;
                }
                c_mut.ability_max_cooldown = scaled_cd;
            }
            if let Some(hook) = ab.script_hook.as_deref().filter(|h| !h.is_empty()) {
                Self::sync_entity_script_ctx_pre_hook(
                    c_mut,
                    Some(eff_target_pos),
                    eff_target_id,
                    None,
                );
                if loaded
                    .script_host
                    .invoke_hook(hook, &mut c_mut.script_ctx)
                    .is_ok()
                {
                    c_mut.health = c_mut.script_ctx.health.clamp(0.0, c_mut.max_health);
                    c_mut.script_ctx.health = c_mut.health;
                    emitted = std::mem::take(&mut c_mut.script_ctx.emitted_actions);
                }
            }
        }
        if !emitted.is_empty() {
            self.execute_emitted_script_actions(Some(caster_id), player_id, emitted, loaded);
        }
        true
    }

    /// Ticks altar/HQ hero revival queues, proximity ground relic pickup, and deterministic auto-cast abilities.
    pub fn step_heroes_and_items(&mut self, loaded: &LoadedMod) {
        let dt = self.dt;
        let is_streaming = loaded.manifest.rules.economy_model == EconomyModel::Streaming;

        // 1. Advance hero revival queues at altars / HQs
        if !self.trait_stores.revival_queues.is_empty() {
            let altar_ids: Vec<u64> = self.trait_stores.revival_queues.keys().copied().collect();
            let mut completed_revivals: Vec<(u8, [i32; 2], HeroInstanceState, String, f32)> =
                Vec::new();
            for bid in altar_ids {
                let Some((pid, b_tile, b_fp)) = self.entities.get(&bid).and_then(|e| {
                    if e.health <= 0.0 || e.construction_progress < 1.0 || e.power_disabled {
                        return None;
                    }
                    Some((e.player_id, e.tile_origin, self.entity_footprint(e, loaded)))
                }) else {
                    continue;
                };
                let speed_factor = self
                    .players
                    .get(&pid)
                    .map(|ec| {
                        let pwr = if ec.is_low_power() || ec.power_sabotage_timer > 0.0 {
                            loaded.manifest.rules.low_power_speed_factor
                        } else {
                            1.0
                        };
                        let stall = if is_streaming {
                            ec.stall_efficiency
                        } else {
                            1.0
                        };
                        pwr * stall
                    })
                    .unwrap_or(1.0);
                let mut finished_job: Option<HeroRevivalJob> = None;
                if let Some(q) = self.trait_stores.revival_queues.get_mut(&bid) {
                    if let Some(front) = q.front_mut() {
                        front.remaining_secs -= dt * speed_factor;
                        if front.remaining_secs <= 0.0 {
                            finished_job = q.pop_front();
                        }
                    }
                    if q.is_empty() {
                        self.trait_stores.revival_queues.remove(&bid);
                    }
                }
                if let Some(job) = finished_job {
                    let exit_tile = [b_tile[0] + (b_fp[0] as i32) / 2, b_tile[1] + b_fp[1] as i32];
                    completed_revivals.push((
                        pid,
                        exit_tile,
                        job.record.hero_state,
                        job.record.unit_id,
                        job.record.experience,
                    ));
                }
            }

            for (pid, exit_tile, saved_state, hero_uid, saved_xp) in completed_revivals {
                let spawn_t = self.grid.find_nearest_walkable(exit_tile);
                if let Some(new_hid) = self.spawn_unit(pid, &hero_uid, spawn_t, loaded) {
                    self.trait_stores.heroes.insert(new_hid, saved_state);
                    if let Some(ent) = self.entities.get_mut(&new_hid) {
                        ent.experience = saved_xp;
                    }
                    self.recompute_hero_stats_and_item_buffs(new_hid, loaded);
                    if let Some(ep) = self.trait_stores.energy_pools.get_mut(&new_hid) {
                        ep.energy = ep.max_energy;
                    }
                    if let Some(ent) = self.entities.get_mut(&new_hid) {
                        ent.health = ent.max_health;
                        ent.energy = ent.max_energy;
                        ent.script_ctx.health = ent.health;
                        ent.script_ctx.energy = ent.energy;
                    }
                }
            }
        }

        // 2. Proximity world relic/item pickup for heroes stepping over `SimGroundItem` drops
        if !self.ground_items.is_empty() && !self.trait_stores.heroes.is_empty() {
            let hero_ids: Vec<u64> = self.trait_stores.heroes.keys().copied().collect();
            for hid in hero_ids {
                let Some(h_pos) = self.entities.get(&hid).and_then(|e| {
                    (e.health > 0.0 && e.transported_by.is_none() && !e.is_phased_out())
                        .then_some(e.position)
                }) else {
                    continue;
                };
                if let Some(gi_idx) = self
                    .ground_items
                    .iter()
                    .position(|gi| det_distance_xz(h_pos, gi.position) <= SimScalar::from_f32(2.6))
                {
                    let _ = self.try_equip_ground_item_index(hid, gi_idx, loaded);
                }
            }
        }

        // 3. Deterministic auto-cast evaluation (`auto_cast_abilities`)
        self.step_auto_cast_abilities(loaded);
    }

    fn step_auto_cast_abilities(&mut self, loaded: &LoadedMod) {
        let mut candidates: Vec<(u64, u8, [f32; 3], Vec<usize>)> = Vec::new();
        for ent in self.entities.values() {
            if ent.auto_cast_abilities.is_empty() || !self.can_entity_act(ent, loaded) {
                continue;
            }
            candidates.push((
                ent.id,
                ent.player_id,
                ent.position,
                ent.auto_cast_abilities.iter().copied().collect(),
            ));
        }
        if candidates.is_empty() {
            return;
        }

        for (cid, pid, caster_pos, ab_indices) in candidates {
            for ab_idx in ab_indices {
                let Some((unit_id_opt, ab_def, caster_hp, caster_max_hp, cur_target)) =
                    self.entities.get(&cid).and_then(|e| {
                        if e.ability_cooldown_at(ab_idx) > 0.0 {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .and_then(|u| u.abilities.get(ab_idx))
                                .map(|ab| {
                                    (
                                        Some(unit_id.clone()),
                                        ab.clone(),
                                        e.health,
                                        e.max_health,
                                        e.target_entity_id,
                                    )
                                }),
                            EntityKind::Building { building_id } => loaded
                                .buildings
                                .get(building_id)
                                .and_then(|b| b.abilities.get(ab_idx))
                                .map(|ab| {
                                    (None, ab.clone(), e.health, e.max_health, e.target_entity_id)
                                }),
                        }
                    })
                else {
                    continue;
                };

                let rank =
                    self.effective_ability_rank(cid, unit_id_opt.as_deref(), &ab_def, loaded);
                if rank == 0 {
                    continue;
                }
                let cur_energy = self
                    .trait_stores
                    .energy_pools
                    .get(&cid)
                    .map(|ep| ep.energy)
                    .unwrap_or(0.0);
                if ab_def.energy_cost > 0.0 && cur_energy + 1e-4 < ab_def.energy_cost {
                    continue;
                }

                let friendly_buff = ab_def.payloads.iter().any(
                    |p| matches!(p, EffectPayload::ApplyAreaStatus { friendly_only, .. } if *friendly_only),
                );
                let search_r = ab_def.range.max(ab_def.radius).max(4.5);

                let chosen_target: Option<([f32; 3], Option<u64>)> = match ab_def.target_mode {
                    AbilityTargetMode::SelfOnly => {
                        let in_danger = cur_target.is_some()
                            || caster_hp < caster_max_hp * 0.92
                            || self.entities.values().any(|other| {
                                other.is_targetable()
                                    && self.are_hostile(pid, other.player_id)
                                    && det_distance_xz(caster_pos, other.position)
                                        <= SimScalar::from_f32(10.0)
                            });
                        in_danger.then_some((caster_pos, Some(cid)))
                    }
                    _ if friendly_buff => self
                        .entities
                        .values()
                        .filter(|other| {
                            other.is_targetable()
                                && !self.are_hostile(pid, other.player_id)
                                && other.health < other.max_health * 0.95
                                && det_distance_xz(caster_pos, other.position)
                                    <= SimScalar::from_f32(search_r)
                        })
                        .min_by_key(|other| det_distance_xz(caster_pos, other.position))
                        .map(|e| (e.position, Some(e.id))),
                    _ => {
                        let explicit_enemy = cur_target.and_then(|tid| {
                            self.entities.get(&tid).and_then(|e| {
                                (e.is_targetable()
                                    && self.are_hostile(pid, e.player_id)
                                    && det_distance_xz(caster_pos, e.position)
                                        <= SimScalar::from_f32(search_r))
                                .then_some((e.position, Some(e.id)))
                            })
                        });
                        explicit_enemy.or_else(|| {
                            self.entities
                                .values()
                                .filter(|other| {
                                    other.is_targetable()
                                        && self.are_hostile(pid, other.player_id)
                                        && det_distance_xz(caster_pos, other.position)
                                            <= SimScalar::from_f32(search_r)
                                })
                                .min_by_key(|other| det_distance_xz(caster_pos, other.position))
                                .map(|e| (e.position, Some(e.id)))
                        })
                    }
                };

                if let Some((t_pos, t_id)) = chosen_target {
                    if self.execute_unit_or_item_ability(
                        pid,
                        cid,
                        Some(ab_idx),
                        &ab_def,
                        rank,
                        t_pos,
                        t_id,
                        loaded,
                    ) {
                        break;
                    }
                }
            }
        }
    }

    #[inline]
    pub(crate) fn sync_entity_script_ctx_pre_hook(
        ent: &mut SimEntity,
        target_pos: Option<[f32; 3]>,
        target_entity_id: Option<u64>,
        attacker_id: Option<u64>,
    ) {
        ent.script_ctx.entity_id = ent.id;
        ent.script_ctx.player_id = ent.player_id;
        ent.script_ctx.self_pos = ent.position;
        ent.script_ctx.health = ent.health;
        ent.script_ctx.max_health = ent.max_health;
        ent.script_ctx.energy = ent.energy;
        ent.script_ctx.max_energy = ent.max_energy;
        ent.script_ctx.target_pos = target_pos;
        ent.script_ctx.target_entity_id = target_entity_id;
        ent.script_ctx.attacker_id = attacker_id;
        ent.script_ctx.emitted_actions.clear();
    }

    pub(crate) fn resolve_status_def_by_id<'a>(
        status_id: &str,
        loaded: &'a LoadedMod,
    ) -> Option<&'a StatusEffectDef> {
        for u in loaded.units.values() {
            for ab in &u.abilities {
                if let Some(ref s) = ab.applies_status {
                    if s.id == status_id {
                        return Some(s);
                    }
                }
                for p in &ab.payloads {
                    if let EffectPayload::ApplyAreaStatus { status, .. } = p {
                        if status.id == status_id {
                            return Some(status);
                        }
                    }
                }
            }
        }
        for w in loaded.weapons.values() {
            for s in w.iter_on_hit_statuses() {
                if s.id == status_id {
                    return Some(s);
                }
            }
        }
        for item in loaded.items.values() {
            if let Some(ref ab) = item.active_ability {
                for p in &ab.payloads {
                    if let EffectPayload::ApplyAreaStatus { status, .. } = p {
                        if status.id == status_id {
                            return Some(status);
                        }
                    }
                }
            }
        }
        None
    }

    /// Executes `ScriptAction` records emitted by `ScriptContext` 2.0 or `WorldEventContext` hooks (`Task 232`).
    pub fn execute_emitted_script_actions(
        &mut self,
        source_entity_id: Option<u64>,
        default_player_id: u8,
        actions: Vec<ScriptAction>,
        loaded: &LoadedMod,
    ) {
        for action in actions {
            match action {
                ScriptAction::ApplyBuff {
                    target_entity_id,
                    status_id,
                    duration_secs,
                    stacks,
                } => {
                    let count = stacks.max(1);
                    let mut buff =
                        if let Some(def) = Self::resolve_status_def_by_id(&status_id, loaded) {
                            let mut b = ActiveBuff::from_def(def);
                            if duration_secs > 0.0 {
                                b.remaining_secs = duration_secs;
                            }
                            b.max_stacks = b.max_stacks.max(count);
                            b
                        } else {
                            ActiveBuff {
                                id: status_id,
                                remaining_secs: duration_secs.max(0.25),
                                speed_multiplier: 1.06,
                                damage_multiplier: 1.08,
                                armor_multiplier: 1.05,
                                stack_policy: StackPolicy::StackIntensity,
                                max_stacks: count.max(3),
                                stacks: 1,
                                dispel_category: Some("magic".to_string()),
                                ..Default::default()
                            }
                        };
                    buff.source_entity_id = source_entity_id;
                    buff.attacker_player_id = default_player_id;
                    if let Some(t_ent) = self.entities.get_mut(&target_entity_id) {
                        for _ in 0..count {
                            t_ent.apply_buff_stack(buff.clone());
                        }
                    }
                }
                ScriptAction::SpawnProjectile {
                    weapon_id,
                    origin,
                    target_pos,
                    target_entity_id,
                } => {
                    let Some(w_def) = loaded.weapons.get(&weapon_id) else {
                        continue;
                    };
                    if w_def.delivery == DeliveryKind::Instant {
                        let rad = w_def.splash_radius.max(1.2);
                        self.execute_effect_payload(
                            default_player_id,
                            source_entity_id,
                            Some(origin),
                            target_pos,
                            &EffectPayload::AreaDamage {
                                radius: rad,
                                damage: w_def.damage,
                                friendly_fire: false,
                            },
                            loaded,
                        );
                    } else {
                        let pid = self.next_projectile_id;
                        self.next_projectile_id += 1;
                        self.projectiles.push(SimProjectile {
                            id: pid,
                            attacker_id: source_entity_id.unwrap_or(0),
                            attacker_player: default_player_id,
                            target_id: target_entity_id.unwrap_or(0),
                            weapon_id,
                            damage: w_def.damage,
                            start_pos: origin,
                            current_pos: origin,
                            target_pos,
                            speed: w_def.projectile_speed.max(8.0),
                            delivery: w_def.delivery,
                            arc_height: w_def.arc_height,
                            progress: 0.0,
                            total_dist: crate::det_math::det_distance_xz_f32(origin, target_pos)
                                .max(0.5),
                            projectile_style: w_def.projectile_style,
                            splash_radius: w_def.splash_radius,
                            gravity: w_def.ballistics.gravity,
                            collides_with_terrain: w_def.ballistics.collides_with_terrain,
                            collides_with_structures: w_def.ballistics.collides_with_structures,
                            ..Default::default()
                        });
                    }
                }
                ScriptAction::SpawnEntity {
                    def_id,
                    player_id,
                    pos,
                    is_building,
                } => {
                    let raw_tile = self.grid.world_to_tile(pos);
                    let tile = self.grid.find_nearest_walkable(raw_tile);
                    if is_building {
                        let _ = self.spawn_building(player_id, &def_id, tile, loaded, false);
                    } else {
                        let _ = self.spawn_unit(player_id, &def_id, tile, loaded);
                    }
                }
                ScriptAction::DealAreaDamage {
                    center,
                    radius,
                    damage,
                    friendly_fire,
                } => {
                    self.execute_effect_payload(
                        default_player_id,
                        source_entity_id,
                        None,
                        center,
                        &EffectPayload::AreaDamage {
                            radius,
                            damage,
                            friendly_fire,
                        },
                        loaded,
                    );
                }
                ScriptAction::IssueCommand {
                    entity_id,
                    command,
                    target_pos,
                    target_entity_id,
                } => {
                    let owner = self
                        .entities
                        .get(&entity_id)
                        .map(|e| e.player_id)
                        .unwrap_or(default_player_id);
                    match command.as_str() {
                        "stop" => self.apply_command(
                            GameCommand::StopUnits {
                                player_id: owner,
                                entity_ids: vec![entity_id],
                            },
                            loaded,
                        ),
                        "hold" | "hold_position" | "hold_ground" => self.apply_command(
                            GameCommand::SetUnitStance {
                                player_id: owner,
                                entity_ids: vec![entity_id],
                                stance: UnitStance::HoldGround,
                            },
                            loaded,
                        ),
                        "move" => {
                            if let Some(tp) = target_pos {
                                self.apply_command(
                                    GameCommand::MoveUnits {
                                        player_id: owner,
                                        entity_ids: vec![entity_id],
                                        target_pos: tp,
                                    },
                                    loaded,
                                );
                            }
                        }
                        "attack_move" => {
                            if let Some(tp) = target_pos {
                                self.apply_command(
                                    GameCommand::AttackMove {
                                        player_id: owner,
                                        entity_ids: vec![entity_id],
                                        target_pos: tp,
                                    },
                                    loaded,
                                );
                            }
                        }
                        "attack" => {
                            if let Some(tid) = target_entity_id {
                                self.apply_command(
                                    GameCommand::AttackTarget {
                                        player_id: owner,
                                        attacker_ids: vec![entity_id],
                                        target_entity_id: tid,
                                    },
                                    loaded,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
