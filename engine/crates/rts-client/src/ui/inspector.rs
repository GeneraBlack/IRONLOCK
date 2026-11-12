use super::*;
use crate::audio::SoundSettingsResource;
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache};
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::UiFontResource;
use crate::AssetRootResource;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeMap;

use super::sidebar::populate_sidebar_buttons;

pub(super) fn build_bottom_inspector_string(
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
) -> String {
    if interaction.selected_ids.is_empty() {
        return "COMMAND READY (NO SELECTION)\n[LMB] Select / Inspect | [RMB] Move / Attack\n[G] Deploy / Ability | [D] Diplomacy | [Y] Tech-Tree | [Tab] Logbook".to_string();
    }
    let first_id = *interaction.selected_ids.iter().next().unwrap();
    let Some(ent) = world.entities.get(&first_id) else {
        return String::new();
    };

    let is_non_owned = ent.player_id != interaction.local_player_id
        && !interaction.replay_active
        && interaction.observer_pov_player_id == 0;
    if is_non_owned {
        let allegiance = if ent.player_id == 0 {
            "NEUTRAL"
        } else if world.are_hostile(interaction.local_player_id, ent.player_id) {
            "HOSTILE"
        } else {
            "ALLIED"
        };
        return match &ent.kind {
            EntityKind::Unit { unit_id } => {
                if let Some(u) = loaded.units.get(unit_id) {
                    let weap_info = u
                        .primary_weapon
                        .as_ref()
                        .and_then(|wid| loaded.weapons.get(wid))
                        .map(|w| format!("{} ({:.0} DMG)", w.name, w.damage))
                        .unwrap_or_else(|| "Unarmed".to_string());
                    format!(
                        "INSPECTED {allegiance} UNIT (P{}): {} ({:?}/{:?})  |  Rank: {:?}  |  HP: {:.0}/{:.0}  |  Weapon: {weap_info}",
                        ent.player_id, u.name, u.role, u.movement_domain, ent.veterancy, ent.health, ent.max_health
                    )
                } else {
                    format!("Inspected {allegiance} Unit #{first_id}")
                }
            }
            EntityKind::Building { building_id } => {
                if let Some(b) = loaded.buildings.get(building_id) {
                    format!(
                        "INSPECTED {allegiance} STRUCTURE (P{}): {} ({:?})  |  HP: {:.0}/{:.0}  |  Footprint: {}x{}",
                        ent.player_id, b.name, b.role, ent.health, ent.max_health, b.footprint[0], b.footprint[1]
                    )
                } else {
                    format!("Inspected {allegiance} Structure #{first_id}")
                }
            }
        };
    }

    let cap = world.carrier_capacity(ent, loaded);
    let cargo_info = if world.is_tunnel_network_building(ent, loaded) {
        let shared_cnt = world
            .players
            .get(&ent.player_id)
            .map(|ec| ec.shared_tunnel_passengers.len())
            .unwrap_or(ent.passenger_ids.len());
        format!("  |  Tunnel Network: {shared_cnt}/{cap} [U Unload]")
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

    match &ent.kind {
        EntityKind::Unit { unit_id } => {
            let Some(u) = loaded.units.get(unit_id) else {
                return format!("Unit #{first_id}");
            };
            let active_primary_wid = if ent.script_ctx.deployed {
                u.deploy_weapon.as_ref().or(u.primary_weapon.as_ref())
            } else {
                u.primary_weapon.as_ref()
            };
            let weap_info = active_primary_wid
                .and_then(|w_id| loaded.weapons.get(w_id))
                .map(|w| format!("{} ({:.0} DMG, {:.0}m)", w.name, w.damage, w.range))
                .unwrap_or_else(|| "Unarmed".to_string());
            let sec_info = u
                .secondary_weapon
                .as_ref()
                .and_then(|w_id| loaded.weapons.get(w_id))
                .map(|w| format!(" + {}", w.name))
                .unwrap_or_default();

            let ability = if let Some(ab_def) = u
                .abilities
                .get(interaction.selected_ability_index)
                .or_else(|| u.abilities.first())
            {
                if u.abilities.len() > 1 {
                    format!(
                        "{} ({}/{})",
                        ab_def.id,
                        interaction
                            .selected_ability_index
                            .min(u.abilities.len() - 1)
                            + 1,
                        u.abilities.len()
                    )
                } else {
                    ab_def.id.clone()
                }
            } else if let Some(b_target) = &u.transforms_into_building {
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
                "INVULNERABLE"
            } else if ent.reverse_moving {
                "REVERSING"
            } else {
                "Ready"
            };

            let mut trait_items: Vec<String> = Vec::new();
            let u_stealth = world.entity_stealth_sensor_spec(ent, loaded);
            let u_pd = world.entity_point_defense_spec(ent, loaded);
            let u_aura = world.entity_support_aura_spec(ent, loaded);
            if u_stealth.cloaked {
                trait_items.push("CLOAK".to_string());
            }
            if u_stealth.detects_cloak {
                trait_items.push("DETECTOR".to_string());
            }
            if u.spy_infiltrator {
                trait_items.push("SPY".to_string());
            }
            if u.plants_c4 {
                trait_items.push("C4-DEMO".to_string());
            }
            if u.mind_control_limit > 0 {
                trait_items.push("MIND-CTRL".to_string());
            }
            if u_aura.mobile_heal_per_sec > 0.0 {
                trait_items.push("MEDIC/REPAIR".to_string());
            }
            if u_pd.intercept_radius > 0.0 {
                trait_items.push("POINT-DEFENSE".to_string());
            }
            if let Some(ref b_spec) = u.builder_trait {
                trait_items.push(format!(
                    "FIELD BUILDER ({} blueprints)",
                    if b_spec.buildable_buildings.is_empty() {
                        loaded.buildings.len()
                    } else {
                        b_spec.buildable_buildings.len()
                    }
                ));
            }
            if let Some(ref g_spec) = u.gatherer_trait {
                trait_items.push(format!(
                    "GATHERER ({})",
                    if g_spec.allowed_resources.is_empty() {
                        "all".to_string()
                    } else {
                        g_spec.allowed_resources.join("/")
                    }
                ));
            }
            if let Some(ref sq) = u.squad_trait {
                let alive = ent.squad_members_alive.max(1);
                let sup_pct =
                    (ent.suppression / sq.max_suppression.max(1.0) * 100.0).clamp(0.0, 100.0);
                trait_items.push(format!(
                    "SQUAD {alive}/{} (Supp {:.0}%)",
                    sq.member_count, sup_pct
                ));
            }
            if let Some(ifv_mode) = world.ifv_passenger_mode_label(ent, loaded) {
                trait_items.push(format!("IFV: {ifv_mode}"));
            }
            for buff in &ent.active_buffs {
                if let Some(label) = buff.display_label.as_ref().filter(|s| !s.is_empty()) {
                    if !trait_items.contains(label) {
                        trait_items.push(label.clone());
                    }
                }
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
                        "GROUP ({} Units) [TAB: {}]: {}  |  ",
                        interaction.selected_ids.len(),
                        sub_lbl,
                        parts.join(", ")
                    );
                }
            }

            format!(
                "{}UNIT: {} ({:?}/{:?})  |  Stance: {}  |  Rank: {:?}  |  HP: {:.0}/{:.0}{}  |  Spd: {:.1}/{:.1}{}{}{}\nWEAPON: {}{}  |  [G]: {} ({})  |  [Space] Event  |  [J] Form  |  [K] Reverse  |  [Z] Patrol  |  [F] Guard  |  [X] Atk-Move",
                squad_header,
                u.name,
                u.role,
                u.movement_domain,
                ent.stance.label(),
                ent.veterancy,
                ent.health,
                ent.max_health,
                ammo_info,
                ent.current_speed,
                u.speed * ent.script_ctx.speed_multiplier * ent.veterancy.speed_bonus(),
                trait_str,
                cargo_info,
                hg_info,
                weap_info,
                sec_info,
                ability,
                status
            )
        }
        EntityKind::Building { building_id } => {
            let Some(b) = loaded.buildings.get(building_id) else {
                return format!("Structure #{first_id}");
            };
            let build_state = if ent.has_c4_bomb() {
                format!("C4 ARMED ({:.1}s)", ent.c4_remaining())
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
                "POWER OFF [O to Enable]".to_string()
            } else if ent.production_paused {
                "PRODUCTION PAUSED [I to Resume]".to_string()
            } else if ent.is_primary {
                "OPERATIONAL [PRIMARY]".to_string()
            } else {
                "OPERATIONAL".to_string()
            };

            let repair_status = if ent.is_repairing {
                "REPAIRING [ACTIVE]"
            } else {
                "Standby"
            };
            let mut special_action = String::new();
            if let Some(sw) = b.superweapon {
                special_action.push_str(&format!("  |  [G] Superweapon ({sw:?})"));
            } else if let Some(u_target) = &b.transforms_into_unit {
                special_action.push_str(&format!("  |  [G] Pack into {u_target}"));
            }
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
                    special_action.push_str(&format!("  |  [N] Upgrade -> {up_name}"));
                }
            }
            let b_container = world.entity_container_spec(ent, loaded);
            let eff_power_delta =
                b.power_delta + b_container.power_per_occupant * (ent.passenger_ids.len() as i32);
            let refund = b.cost / 2;

            format!(
                "STRUCTURE: {} ({:?})  |  Status: {}  |  HP: {:.0}/{:.0}  |  Power: {:+}  |  Footprint: {}x{}{}\nACTIONS:  [B] Set Primary  |  [I] Pause Queue  |  [R] Repair ({})  |  [Delete] Sell (+${}){}  |  [RMB] Rally Point",
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
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_sidebar_and_studio_lists(
    commands: &mut Commands,
    asset_server: Option<&AssetServer>,
    asset_root: &AssetRootResource,
    ui_cache: &mut UiAssetCache,
    ui_font: Option<&UiFontResource>,
    sound_res: &SoundSettingsResource,
    video_res: Option<&VideoPlaybackResource>,
    post_game_graph: Option<Handle<Image>>,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    pid: u8,
    last_sig: &mut LastSidebarSignature,
    container_q: &Query<Entity, With<DynamicButtonsContainer>>,
    studio_mods_q: &Query<Entity, With<StudioModListContainer>>,
) {
    if interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open
    {
        let avail_buildings = world.available_buildings_for_player(pid, loaded);
        let avail_units = world.available_units_for_player(pid, loaded);
        let avail_upgrades = world.available_upgrades_for_player(pid, loaded);
        let avail_support_powers = world.available_support_powers_for_player(pid, loaded);

        // Collect Worker On-Site Construction blueprints when owned `BuilderTraitSpec` workers are selected
        let mut worker_buildable_ids: Vec<String> = Vec::new();
        for sid in &interaction.selected_ids {
            if let Some(ent) = world.entities.get(sid) {
                if ent.player_id == pid && ent.transported_by.is_none() {
                    if let EntityKind::Unit { unit_id } = &ent.kind {
                        if let Some(u_def) = loaded.units.get(unit_id) {
                            if let Some(ref builder) = u_def.builder_trait {
                                let candidates: Vec<String> =
                                    if builder.buildable_buildings.is_empty() {
                                        avail_buildings.clone()
                                    } else {
                                        builder.buildable_buildings.clone()
                                    };
                                for bid in candidates {
                                    if loaded.buildings.contains_key(&bid)
                                        && !worker_buildable_ids.contains(&bid)
                                    {
                                        worker_buildable_ids.push(bid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let sp_sig: String = avail_support_powers
            .iter()
            .map(|(sp, rem)| {
                format!(
                    "{}:{}",
                    sp.id,
                    if *rem <= 0.0 { 0 } else { rem.ceil() as u32 }
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        let econ_opt = world.players.get(&pid);
        let ready_b_id = econ_opt.and_then(|e| e.ready_building_id.clone());
        let bq_sig = econ_opt
            .and_then(|e| {
                e.building_queue.as_ref().map(|(id, rem, total)| {
                    let pct = (((1.0 - rem / total.max(0.1)) * 100.0).clamp(0.0, 100.0) as u32) / 2;
                    format!("{id}:{pct}:{}", e.building_queue_paused)
                })
            })
            .unwrap_or_default();
        let active_up_sig = econ_opt
            .and_then(|e| {
                e.active_upgrade.as_ref().map(|(id, rem, total)| {
                    let pct = (((1.0 - rem / total.max(0.1)) * 100.0).clamp(0.0, 100.0) as u32) / 2;
                    format!("{id}:{pct}")
                })
            })
            .unwrap_or_default();
        let pwr_bucket = econ_opt
            .map(|e| {
                let net = e.power_produced.saturating_sub(e.power_consumed);
                format!("{}:{}:{}", net / 10, e.is_low_power(), e.promotion_points)
            })
            .unwrap_or_default();

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
        let mut unit_head_progress: BTreeMap<String, (u32, bool)> = BTreeMap::new();
        for ent in world.entities.values() {
            if ent.player_id == pid {
                for (idx, item) in ent.production_queue.iter().enumerate() {
                    *unit_queue_counts.entry(item.unit_id.clone()).or_insert(0) += 1;
                    if idx == 0 {
                        let pct = ((1.0 - item.remaining_secs / item.total_secs.max(0.1)) * 100.0)
                            .clamp(0.0, 99.0) as u32;
                        let item_paused = ent.production_paused || item.on_hold;
                        unit_head_progress
                            .entry(item.unit_id.clone())
                            .and_modify(|(best_pct, paused)| {
                                if pct > *best_pct {
                                    *best_pct = pct;
                                }
                                *paused = *paused && item_paused;
                            })
                            .or_insert((pct, item_paused));
                    }
                }
            }
        }
        let queue_counts_sig: String = unit_queue_counts
            .iter()
            .map(|(k, v)| {
                let (pct, paused) = unit_head_progress.get(k).copied().unwrap_or((0, false));
                format!("{k}={v}:{}:{}", pct / 2, paused)
            })
            .collect::<Vec<_>>()
            .join(",");

        let sidebar_sig = format!(
            "mod={}:p{pid}:hud={:?}:tab={:?}:pages={:?}:vdom={:?}:sp_p={}:up_p={}:pwr={pwr_bucket}:place={:?}:{}:wplace={:?}:wb={}:rc={}:sc={}:hc={}:pc={}:sp={}:{:?}:ready={:?}:bq={bq_sig}:aup={active_up_sig}:uc={}:b={}:u={}:up={}:next={}:grid={}:lang={}",
            loaded.manifest.id,
            interaction.effective_hud_layout(loaded),
            interaction.active_sidebar_tab,
            interaction.sidebar_tab_pages,
            interaction.vehicle_domain_filter,
            interaction.support_power_page,
            interaction.upgrade_page,
            interaction.placing_building_id,
            interaction.placement_rotated,
            interaction.worker_placing_building,
            worker_buildable_ids.join(","),
            interaction.repair_cursor_mode,
            interaction.sell_cursor_mode,
            interaction.harvest_cursor_mode,
            interaction.ping_cursor_mode,
            sp_sig,
            interaction.active_support_power_id,
            ready_b_id,
            queue_counts_sig,
            avail_buildings.join(","),
            avail_units.join(","),
            avail_upgrades.join(","),
            has_next_campaign_mission,
            interaction.grid_hotkeys_enabled,
            interaction.language
        );

        if sidebar_sig != last_sig.sidebar_sig {
            last_sig.sidebar_sig = sidebar_sig;
            if let Ok(container_ent) = container_q.get_single() {
                commands.entity(container_ent).despawn_descendants();
                populate_sidebar_buttons(
                    commands,
                    asset_server,
                    asset_root,
                    ui_cache,
                    container_ent,
                    world,
                    loaded,
                    interaction,
                    pid,
                    has_next_campaign_mission,
                    &worker_buildable_ids,
                    &avail_support_powers,
                    &avail_buildings,
                    &avail_units,
                    &avail_upgrades,
                    &unit_queue_counts,
                    &unit_head_progress,
                );
            }
        }
    }

    if (interaction.menu_open || interaction.app_phase == AppPhase::PostGameScore)
        && !interaction.studio_open
    {
        super::sidebar_drawers::rebuild_player_menu_modal(
            commands,
            asset_server,
            asset_root,
            ui_cache,
            ui_font,
            sound_res,
            video_res,
            post_game_graph,
            world,
            loaded,
            interaction,
            last_sig,
            studio_mods_q,
            pid,
        );
    }
}
