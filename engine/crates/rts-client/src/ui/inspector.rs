use super::*;
use crate::input::ClientInteractionState;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeMap;

use super::sidebar::{populate_sidebar_buttons, rebuild_studio_modal_list};

pub(super) fn build_bottom_inspector_string(
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) -> String {
    if interaction.selected_ids.is_empty() {
        return if interaction.dev_mode_enabled {
            "TACTICAL COMMAND READY (NO SELECTION)\n[LMB] Select | [RMB] Order | [J] Formation\n[WASD] Pan | [F9] Dev-Console | [F12] Player Mode".to_string()
        } else {
            "COMMAND READY (NO SELECTION)\n[LMB] Select | [RMB] Move / Attack\n[G] Deploy / MCV | [J] Formation | [ESC] Menu".to_string()
        };
    }
    let first_id = *interaction.selected_ids.iter().next().unwrap();
    if let Some(ent) = world.entities.get(&first_id) {
                let cap = world.carrier_capacity(ent, loaded);
                let cargo_info = if world.is_tunnel_network_building(ent, loaded) {
                    let shared_cnt = world
                        .players
                        .get(&ent.player_id)
                        .map(|ec| ec.shared_tunnel_passengers.len())
                        .unwrap_or(ent.passenger_ids.len());
                    format!("  |  Tunnel Network: {shared_cnt}/{cap} [U Unload Here]")
                } else if cap > 0 {
                    format!("  |  Cargo: {}/{} [U Unload]", ent.passenger_ids.len(), cap)
                } else {
                    String::new()
                };
                let hg_info = if ent.position[1] > 1.0 && ent.position[1] < 2.2 {
                    "  |  [HIGH GROUND +20% Range]"
                } else {
                    ""
                };
                let ammo_info = if ent.max_ammo > 0 {
                    format!("  |  [AMMO {}/{}]", ent.current_ammo, ent.max_ammo)
                } else {
                    String::new()
                };
                let detail = match &ent.kind {
                    EntityKind::Unit { unit_id } => {
                        if let Some(u) = loaded.units.get(unit_id) {
                            let active_primary_wid = if ent.script_ctx.deployed {
                                u.deploy_weapon.as_ref().or(u.primary_weapon.as_ref())
                            } else {
                                u.primary_weapon.as_ref()
                            };
                            let weap_info = active_primary_wid
                                .and_then(|w_id| loaded.weapons.get(w_id))
                                .map(|w| {
                                    format!(
                                        "{} ({:.0} DMG, {:.0}m Range)",
                                        w.name, w.damage, w.range
                                    )
                                })
                                .unwrap_or_else(|| "Unarmed".to_string());
                            let sec_info = u
                                .secondary_weapon
                                .as_ref()
                                .and_then(|w_id| loaded.weapons.get(w_id))
                                .map(|w| format!(" + {}", w.name))
                                .unwrap_or_default();
                            let ability = if let Some(b_target) = &u.transforms_into_building {
                                format!("Transform -> {b_target}")
                            } else if u.deploy_weapon.is_some() || u.immobile_when_deployed {
                                "Deploy / Siege Mode".to_string()
                            } else {
                                u.ability_id
                                    .clone()
                                    .unwrap_or_else(|| "Deploy/Stance".to_string())
                            };
                            let status = if ent.emp_remaining() > 0.0 {
                                "EMP DISABLED!"
                            } else if ent.script_ctx.deployed {
                                "DEPLOYED"
                            } else if ent.is_invulnerable() {
                                ent.active_buffs
                                    .iter()
                                    .find_map(|b| {
                                        b.invulnerable
                                            .then_some(b.display_label.as_deref())
                                            .flatten()
                                            .filter(|s| !s.is_empty())
                                    })
                                    .unwrap_or("INVULNERABLE")
                            } else if ent.reverse_moving {
                                "REVERSING"
                            } else {
                                "Ready"
                            };
                            let u_container = world.entity_container_spec(ent, loaded);
                            let u_stealth = world.entity_stealth_sensor_spec(ent, loaded);
                            let u_death = world.entity_death_payload_spec(ent, loaded);
                            let u_pd = world.entity_point_defense_spec(ent, loaded);
                            let u_aura = world.entity_support_aura_spec(ent, loaded);
                            let mut traits = Vec::new();
                            if u_stealth.cloaked {
                                traits.push("CLOAK");
                            }
                            if u.spy_infiltrator {
                                traits.push("SPY");
                            }
                            if u.plants_c4 {
                                if u.movement_domain == rts_sdk::schema::MovementDomain::Amphibious {
                                    traits.push("NAVAL & BUILDING C4 SABOTEUR");
                                } else {
                                    traits.push("C4-DEMO");
                                }
                            }
                            if u.chrono_harvester {
                                traits.push("CHRONO-HARV");
                            }
                            if u.mind_control_limit > 0 {
                                traits.push("MIND-CTRL");
                            }
                            if u.crushes_vehicles {
                                traits.push("CRUSHES-VEHICLES");
                            } else if u.crushes_infantry {
                                traits.push("CRUSHER");
                            }
                            if u_container.passengers_can_fire {
                                traits.push("MOBILE-FIREPORTS");
                            }
                            if u.spawns_drone_unit.is_some() {
                                traits.push("CARRIER-DRONES");
                            }
                            if u_stealth.detects_cloak {
                                traits.push("DETECTOR");
                            }
                            if u_aura.mobile_heal_per_sec > 0.0 {
                                traits.push("MEDIC/REPAIR");
                            }
                            if ent.script_ctx.damage_multiplier > 1.01 {
                                traits.push("CRATE DMG+");
                            }
                            if ent.salvage_upgraded {
                                traits.push("SALVAGE+");
                            }
                            if ent.permanent_cloak {
                                traits.push("STEALTH+");
                            }
                            if u.subterranean {
                                if ent.burrowed {
                                    traits.push("BURROWED");
                                } else {
                                    traits.push("SUBTERRANEAN");
                                }
                            }
                            if u.kamikaze {
                                traits.push("KAMIKAZE");
                            }
                            if u_stealth.disguises_as_terrain {
                                traits.push("TERRAIN-MIRAGE");
                            }
                            if u.chrono_locomotion {
                                traits.push("CHRONO-PHASED");
                            }
                            if u_pd.intercept_radius > 0.0 {
                                traits.push("POINT-DEFENSE LASER");
                            }
                            if ent.temporal_progress > 0.0 && !ent.is_phased_out() {
                                traits.push("TEMPORAL-LOCKED!");
                            }
                            if world.is_entity_control_offline(ent, loaded) {
                                traits.push("ROBOT-LINK OFFLINE!");
                            }
                            if u.immune_to_mind_control {
                                traits.push("MIND-CTRL IMMUNE");
                            }
                            if u.immune_to_radiation {
                                traits.push("RAD-IMMUNE");
                            }
                            if u.mobile_refinery {
                                traits.push("MOBILE REFINERY");
                            }
                            if u.lands_when_deployed {
                                if ent.script_ctx.deployed {
                                    traits.push("SIEGE ARTILLERY LANDED");
                                } else {
                                    traits.push("AERIAL SIEGE CHOPPER");
                                }
                            }
                            let mut trait_items: Vec<String> =
                                traits.into_iter().map(|s| s.to_string()).collect();
                            for buff in &ent.active_buffs {
                                if let Some(label) = buff.display_label.as_ref().filter(|s| !s.is_empty()) {
                                    if !trait_items.contains(label) {
                                        trait_items.push(label.clone());
                                    }
                                }
                            }
                            if ent.has_c4_bomb() {
                                trait_items.push(format!(
                                    "TIME-BOMB ATTACHED: {:.1}s [Defuse w/ Engineer!]",
                                    ent.c4_remaining()
                                ));
                            }
                            if ent.spin_up_level > 0.05 {
                                trait_items.push(format!(
                                    "GATLING SPIN-UP: {:.1}x ROF",
                                    1.0 + ent.spin_up_level
                                ));
                            }
                            if u.mind_control_overload_dps > 0.0 {
                                trait_items.push(format!(
                                    "PSI-OVERLOAD CAP: {} (+{:.0} DPS/extra)",
                                    u.mind_control_limit, u.mind_control_overload_dps
                                ));
                            }
                            if u.psi_wave_radius > 0.0 && u.psi_wave_damage > 0.0 {
                                trait_items.push(format!(
                                    "PSI-WAVE BLAST: {:.0} DMG / {:.1}m",
                                    u.psi_wave_damage, u.psi_wave_radius
                                ));
                            }
                            if u.garrison_damage_multiplier > 1.0 {
                                trait_items.push(format!(
                                    "GARRISON PYRO: +{:.0}% Bunker DMG",
                                    (u.garrison_damage_multiplier - 1.0) * 100.0
                                ));
                            }
                            if ent.reveal_timer > 0.0 {
                                trait_items.push(format!(
                                    "REVEALED (FIRING: {:.1}s)",
                                    ent.reveal_timer
                                ));
                            }
                            if u.self_heal_per_sec > 0.0 {
                                trait_items.push(format!(
                                    "SELF-HEAL: +{:.0} HP/s",
                                    u.self_heal_per_sec
                                ));
                            }
                            if ent.veterancy == rts_simulation::world::VeterancyRank::Elite {
                                trait_items.push("ELITE SELF-REPAIR (+10 HP/s, +20% RNG/ROF)".to_string());
                                if let Some(ew_id) = &u.elite_weapon {
                                    let ew_name = loaded
                                        .weapons
                                        .get(ew_id)
                                        .map(|w| w.name.as_str())
                                        .unwrap_or(ew_id);
                                    trait_items.push(format!("ELITE WEAPON: {ew_name}"));
                                }
                            }
                            if let Some((_, Some(p_uid), _)) = ent.parasite_info() {
                                trait_items.push(format!("PARASITE HOST: {p_uid}"));
                            }
                            if ent.mind_controlled_by.is_some() {
                                trait_items
                                    .push("MIND-CONTROLLED (100% Grinder/Vats Refund)".to_string());
                            }
                            if let Some((disg_uid, disg_pid)) = &ent.disguised_as {
                                let disg_name = loaded
                                    .units
                                    .get(disg_uid)
                                    .map(|du| du.name.as_str())
                                    .unwrap_or(disg_uid);
                                trait_items.push(format!("DISGUISED AS: {disg_name} (P{disg_pid})"));
                            }
                            if let Some(ifv_mode) = world.ifv_passenger_mode_label(ent, loaded) {
                                trait_items.push(format!("IFV-TURRET: {ifv_mode}"));
                            }
                            if let Some(pw_id) = active_primary_wid {
                                if let Some(pw) = loaded.weapons.get(pw_id) {
                                    if pw.refraction_targets > 0 {
                                        trait_items.push(format!(
                                            "PRISM REFRACTION: {}x ({:.0}% DMG)",
                                            pw.refraction_targets,
                                            pw.refraction_damage_ratio * 100.0
                                        ));
                                    }
                                    if pw.can_mind_control_buildings {
                                        trait_items.push("MIND CONTROLS BUILDINGS".to_string());
                                    }
                                    if pw.naval_only && u.secondary_weapon.is_some() {
                                        trait_items
                                            .push("NAVAL TORPEDO + CRUISE MISSILE".to_string());
                                    }
                                    if pw.min_range > 0.0 {
                                        trait_items
                                            .push(format!("MIN RANGE: {:.1}", pw.min_range));
                                    }
                                }
                            }
                            if let Some(drone_uid) = &u.spawns_drone_unit {
                                let drone_name = loaded
                                    .units
                                    .get(drone_uid)
                                    .map(|du| du.name.as_str())
                                    .unwrap_or(drone_uid.as_str());
                                trait_items.push(format!(
                                    "DRONE WING: {} ({}x)",
                                    drone_name,
                                    u.max_drones.max(1)
                                ));
                            }
                            if u_stealth.cloaked
                                && u.movement_domain == rts_sdk::schema::MovementDomain::Air
                                && u_container.capacity > 0
                            {
                                trait_items.push("STEALTH RADAR TRANSPORT".to_string());
                            }
                            if let Some(dw_id) = &u.deploy_weapon {
                                let dw_name = loaded
                                    .weapons
                                    .get(dw_id)
                                    .map(|w| w.name.as_str())
                                    .unwrap_or(dw_id);
                                trait_items.push(format!("DEPLOY WEAPON: {dw_name}"));
                            }
                            if u.movement_domain == rts_sdk::schema::MovementDomain::Amphibious
                                && u_container.accepts_vehicles
                            {
                                trait_items.push("AMPHIBIOUS VEHICLE TRANSPORT".to_string());
                            }
                            if u_death.explosion_damage > 0.0 {
                                trait_items.push(format!(
                                    "VOLATILE DEATH EXPLOSION ({:.0} DMG)",
                                    u_death.explosion_damage
                                ));
                            }
                            let trait_str = if trait_items.is_empty() {
                                String::new()
                            } else {
                                format!("  |  Traits: [{}]", trait_items.join(", "))
                            };
                            let mut squad_header = String::new();
                            if interaction.selected_ids.len() > 1 {
                                let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                                for sid in &interaction.selected_ids {
                                    if let Some(se) = world.entities.get(sid) {
                                        if let EntityKind::Unit { unit_id: suid } = &se.kind {
                                            *counts.entry(suid.clone()).or_insert(0) += 1;
                                        }
                                    }
                                }
                                if counts.len() > 1 {
                                    let parts: Vec<String> = counts
                                        .iter()
                                        .map(|(uid, cnt)| {
                                            let dname = loaded
                                                .units
                                                .get(uid)
                                                .map(|u| u.name.as_str())
                                                .unwrap_or(uid);
                                            format!("{cnt}x {dname}")
                                        })
                                        .collect();
                                    let sub_lbl = interaction
                                        .active_subgroup_unit_id
                                        .as_deref()
                                        .unwrap_or("ALL");
                                    squad_header = format!(
                                        "SQUAD ({} Units) [TAB Sub-Group: {}]: {}  |  ",
                                        interaction.selected_ids.len(),
                                        sub_lbl,
                                        parts.join(", ")
                                    );
                                }
                            }
                            format!(
                                "{}SELECTED UNIT: {} ({:?}/{:?})  |  Stance: {} [T]  |  Rank: {:?}  |  HP: {:.0}/{:.0}{}  |  Speed: {:.1}{}{}{}\nWEAPON: {}{}  |  [G/Space]: {} ({})  |  [J] Form  |  [K] Reverse  |  [Z] Patrol  |  [F] Guard  |  [X] Atk-Move",
                                squad_header,
                                u.name,
                                u.role,
                                u.movement_domain,
                                ent.stance.label(),
                                ent.veterancy,
                                ent.health,
                                ent.max_health,
                                ammo_info,
                                u.speed * ent.script_ctx.speed_multiplier * ent.veterancy.speed_bonus(),
                                trait_str,
                                cargo_info,
                                hg_info,
                                weap_info,
                                sec_info,
                                ability,
                                status
                            )
                        } else {
                            format!("Unit #{first_id}")
                        }
                    }
                    EntityKind::Building { building_id } => {
                        if let Some(b) = loaded.buildings.get(building_id) {
                            let build_state = if ent.has_c4_bomb() {
                                format!(
                                    "C4 ARMED ({:.1}s - DEFUSE WITH ENGINEER!)",
                                    ent.c4_remaining()
                                )
                            } else if ent.pending_airstrike_timer > 0.0 {
                                format!(
                                    "AIRSTRIKE LOCKED ({:.1}s!)",
                                    ent.pending_airstrike_timer
                                )
                            } else if ent.emp_remaining() > 0.0 {
                                format!("EMP DISABLED ({:.1}s)", ent.emp_remaining())
                            } else if ent.construction_progress < 1.0 {
                                format!("CONSTRUCTING ({:.0}%)", ent.construction_progress * 100.0)
                            } else if let Some((target_b_id, rem_secs, total_secs)) = &ent.upgrading_to {
                                let u_pct = ((1.0 - rem_secs / total_secs.max(0.1)) * 100.0).clamp(0.0, 100.0);
                                let t_name = loaded
                                    .buildings
                                    .get(target_b_id)
                                    .map(|tb| tb.name.as_str())
                                    .unwrap_or(target_b_id);
                                format!("UPGRADING TO {t_name} ({u_pct:.0}%)")
                            } else if ent.power_disabled {
                                "POWER OFF [Press O to Enable]".to_string()
                            } else if ent.production_paused {
                                "PRODUCTION PAUSED [Press I to Resume]".to_string()
                            } else if ent.health < ent.max_health * 0.25 {
                                "CRITICAL DMG [-25% PROD]".to_string()
                            } else if ent.is_primary {
                                "OPERATIONAL [★ PRIMARY]".to_string()
                            } else {
                                "OPERATIONAL".to_string()
                            };
                            let repair_status = if ent.is_repairing {
                                "REPAIRING [ACTIVE]"
                            } else {
                                "Standby"
                            };
                            let mut special_action = if let Some(sw) = b.superweapon {
                                format!("  |  [G/Space] Fire Superweapon ({sw:?})")
                            } else if let Some(u_target) = &b.transforms_into_unit {
                                format!("  |  [G/Space] Pack into {u_target}")
                            } else {
                                String::new()
                            };
                            if b.power_delta < 0 {
                                special_action.push_str(if ent.power_disabled {
                                    "  |  [O] Power ON"
                                } else {
                                    "  |  [O] Power OFF"
                                });
                            }
                            if let Some(up_target) = &b.upgrades_to {
                                if ent.upgrading_to.is_none() {
                                    let up_name = loaded
                                        .buildings
                                        .get(up_target)
                                        .map(|tb| tb.name.as_str())
                                        .unwrap_or(up_target);
                                    let up_cost = if b.upgrade_cost > 0 {
                                        b.upgrade_cost
                                    } else {
                                        loaded
                                            .buildings
                                            .get(up_target)
                                            .map(|tb| (tb.cost - b.cost).max(200))
                                            .unwrap_or(400)
                                    };
                                    special_action.push_str(&format!(
                                        "  |  [Y] Upgrade -> {up_name} (${up_cost})"
                                    ));
                                }
                            }
                            let b_container = world.entity_container_spec(ent, loaded);
                            let b_stealth = world.entity_stealth_sensor_spec(ent, loaded);
                            let b_death = world.entity_death_payload_spec(ent, loaded);
                            let b_pd = world.entity_point_defense_spec(ent, loaded);
                            let b_aura = world.entity_support_aura_spec(ent, loaded);
                            if b_stealth.cloak_field_radius > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [GAP CLOAK FIELD: {:.0}m]",
                                    b_stealth.cloak_field_radius
                                ));
                            }
                            if b_aura.ore_refine_multiplier > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [ORE PURIFIER: +{:.0}%]",
                                    b_aura.ore_refine_multiplier * 100.0
                                ));
                            }
                            if b_container.tunnel_network {
                                special_action.push_str("  |  [SHARED TUNNEL NETWORK]");
                            }
                            if b.grinder {
                                special_action.push_str("  |  [GRINDER: 80% Unit/Vehicle Recycle]");
                            }
                            if b.cloning_vats {
                                special_action.push_str(
                                    "  |  [CLONING VATS: Free Infantry Twin + Recycle (100% Mind-Ctrl)]",
                                );
                            }
                            if ent.has_power_charge_boost() {
                                special_action.push_str(&format!(
                                    "  |  [OVERCHARGED: +35% DMG / +2.0m Range ({:.1}s)]",
                                    ent.power_charge_remaining()
                                ));
                            }
                            if !b.stolen_tech_units.is_empty() {
                                special_action.push_str(&format!(
                                    "  |  [STOLEN TECH LAB: {}]",
                                    b.stolen_tech_units.join(", ")
                                ));
                            }
                            if let Some(p_econ) = world.players.get(&ent.player_id) {
                                if b.role == rts_sdk::schema::BuildingRole::Barracks
                                    && p_econ.veteran_infantry_unlocked
                                {
                                    special_action.push_str("  |  [VET INFANTRY UNLOCKED]");
                                } else if b.role == rts_sdk::schema::BuildingRole::WarFactory
                                    && p_econ.veteran_vehicles_unlocked
                                {
                                    special_action.push_str("  |  [VET VEHICLES UNLOCKED]");
                                }
                                if !p_econ.stolen_tech_units.is_empty()
                                    && matches!(
                                        b.role,
                                        rts_sdk::schema::BuildingRole::Barracks
                                            | rts_sdk::schema::BuildingRole::WarFactory
                                            | rts_sdk::schema::BuildingRole::TechCenter
                                    )
                                {
                                    special_action.push_str(&format!(
                                        "  |  [STOLEN TECH UNLOCKED: {}]",
                                        p_econ.stolen_tech_units.len()
                                    ));
                                }
                            }
                            if b_aura.vehicle_cost_discount > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [INDUSTRIAL PLANT: -{:.0}% Vehicle Cost/Time]",
                                    b_aura.vehicle_cost_discount * 100.0
                                ));
                            }
                            if b_pd.intercept_radius > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [POINT-DEFENSE LASER: {:.0}m]",
                                    b_pd.intercept_radius
                                ));
                            }
                            if b_container.power_per_occupant > 0 {
                                special_action.push_str(&format!(
                                    "  |  [BIO-REACTOR BOOST: +{} Power ({}x +{})]",
                                    b_container.power_per_occupant * (ent.passenger_ids.len() as i32),
                                    ent.passenger_ids.len(),
                                    b_container.power_per_occupant
                                ));
                            }
                            if b_container.accepts_vehicles {
                                special_action.push_str(
                                    "  |  [TANK BUNKER: Vehicle Fireport (+20% DMG / +2.5m Range)]",
                                );
                            } else if b_container.passengers_can_fire {
                                special_action
                                    .push_str("  |  [BUNKER FIREPORTS: Infantry Fire Own Weapons]");
                            }
                            if b_death.explosion_damage > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [MELTDOWN HAZARD: {:.0} DMG / {:.1}m]",
                                    b_death.explosion_damage, b_death.explosion_radius
                                ));
                            }
                            if b_aura.repairs_ships_per_sec > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [NAVAL REPAIR DOCK: +{:.0} HP/s]",
                                    b_aura.repairs_ships_per_sec
                                ));
                            }
                            if !b.unlocks_units.is_empty() {
                                special_action.push_str(&format!(
                                    "  |  [SECRET LAB UNLOCKS: {}]",
                                    b.unlocks_units.join(", ")
                                ));
                            }
                            if b_stealth.psychic_sensor_radius > 0.0 {
                                special_action.push_str(&format!(
                                    "  |  [PSYCHIC SENSOR: {:.0}m Threat Warning]",
                                    b_stealth.psychic_sensor_radius
                                ));
                            }
                            let eff_power_delta = b.power_delta
                                + b_container.power_per_occupant * (ent.passenger_ids.len() as i32);
                            let refund = b.cost / 2;
                            format!(
                                "SELECTED STRUCTURE: {} ({:?})  |  Status: {}  |  HP: {:.0}/{:.0}  |  Power: {:+}  |  Footprint: {}x{}{}\nACTIONS:  [B] Set Primary  |  [I] Pause Queue  |  [R] Repair ({})  |  [Delete] Sell (+${}){}  |  [RMB / Shift+RMB] Rally Chain",
                                b.name,
                                b.role,
                                build_state,
                                ent.health,
                                ent.max_health,
                                eff_power_delta,
                                b.footprint[0],
                                b.footprint[1],
                                cargo_info,
                                repair_status,
                                refund,
                                special_action
                            )
                        } else {
                            format!("Structure #{first_id}")
                        }
                    }
                };
        detail
    } else {
        String::new()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_sidebar_and_studio_lists(
    commands: &mut Commands,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    last_sig: &mut LastSidebarSignature,
    container_q: &Query<Entity, With<DynamicButtonsContainer>>,
    studio_mods_q: &Query<Entity, With<StudioModListContainer>>,
) {
    let pid = interaction.local_player_id;
    // 4. Rebuild Right Sidebar Dynamic Buttons when unlocked tech changes
    let avail_buildings = world.available_buildings_for_player(pid, loaded);
    let avail_units = world.available_units_for_player(pid, loaded);
    let avail_upgrades = world.available_upgrades_for_player(pid, loaded);
    let avail_support_powers = world.available_support_powers_for_player(pid, loaded);
    let sp_sig: String = avail_support_powers
        .iter()
        .map(|(sp, rem)| {
            let r_sec = if *rem <= 0.0 {
                0
            } else {
                rem.ceil() as u32
            };
            format!("{}:{r_sec}", sp.id)
        })
        .collect::<Vec<_>>()
        .join(",");
    let ready_b_id = world
        .players
        .get(&pid)
        .and_then(|e| e.ready_building_id.clone());
    let queued_b_id = world
        .players
        .get(&pid)
        .and_then(|e| e.building_queue.as_ref().map(|(id, _, _)| id.clone()));
    let has_next_campaign_mission = world.winner_player_id == Some(pid)
        && interaction
            .active_campaign_id
            .as_ref()
            .zip(interaction.active_mission_id.as_ref())
            .and_then(|(cid, mid)| {
                loaded
                    .campaigns
                    .get(cid)
                    .and_then(|c| c.missions.iter().find(|m| &m.id == mid))
            })
            .and_then(|m| m.next_mission_id.as_ref())
            .is_some();

    let mut unit_queue_counts: BTreeMap<String, usize> = BTreeMap::new();
    for ent in world.entities.values() {
        if ent.player_id == pid {
            for item in &ent.production_queue {
                *unit_queue_counts.entry(item.unit_id.clone()).or_insert(0) += 1;
            }
        }
    }
    let queue_counts_sig: String = unit_queue_counts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");

    let mut selected_subgroups: BTreeMap<String, usize> = BTreeMap::new();
    if interaction.selected_ids.len() > 1 {
        for sid in &interaction.selected_ids {
            if let Some(se) = world.entities.get(sid) {
                if let EntityKind::Unit { unit_id } = &se.kind {
                    *selected_subgroups.entry(unit_id.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    let subgroups_sig: String = if selected_subgroups.len() > 1 {
        selected_subgroups
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(",")
    } else {
        String::new()
    };

    let promo_pts = world
        .players
        .get(&pid)
        .map(|e| e.promotion_points)
        .unwrap_or(0);
    let sidebar_sig = format!(
        "mod={}:p{pid}:dev={}:tab={:?}:pp={promo_pts}:place={:?}:{}:sp={}:{:?}:ready={:?}:q={:?}:uc={}:sub={}:{:?}:b={}:u={}:up={}:rep={}:{}:{}:next={}",
        loaded.manifest.id,
        interaction.dev_mode_enabled,
        interaction.active_sidebar_tab,
        interaction.placing_building_id,
        interaction.placement_rotated,
        sp_sig,
        interaction.active_support_power_id,
        ready_b_id,
        queued_b_id,
        queue_counts_sig,
        subgroups_sig,
        interaction.active_subgroup_unit_id,
        avail_buildings.join(","),
        avail_units.join(","),
        avail_upgrades.join(","),
        interaction.replay_active,
        interaction.observer_pov_player_id,
        interaction.replay_total_ticks,
        has_next_campaign_mission
    );

    if sidebar_sig != last_sig.sidebar_sig {
        last_sig.sidebar_sig = sidebar_sig;
        if let Ok(container_ent) = container_q.get_single() {
            commands.entity(container_ent).despawn_descendants();
            populate_sidebar_buttons(
                commands,
                asset_server,
                container_ent,
                world,
                loaded,
                interaction,
                pid,
                has_next_campaign_mission,
                &selected_subgroups,
                &avail_support_powers,
                &avail_buildings,
                &avail_units,
                &avail_upgrades,
                &unit_queue_counts,
            );
        }
    }
    rebuild_studio_modal_list(
        commands,
        asset_server,
        asset_root,
        loaded,
        interaction,
        last_sig,
        studio_mods_q,
        pid,
    );
}
