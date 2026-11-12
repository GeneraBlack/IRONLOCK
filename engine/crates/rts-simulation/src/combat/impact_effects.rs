//! Post-impact world mutations: bounties, Magnetron drops, siphons, radiation, props, bridges, and `ScriptContext` 2.0 hooks (`Task 232` / `234b`).

use super::{ballistics, PendingAttack};
use crate::world::{EntityKind, RadiationZone, SimWorld, VeterancyRank};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitRole};

pub(super) struct PostImpactContext {
    pub target_killed: bool,
    pub killed_unit: bool,
    pub killed_building: bool,
    pub splash_kills_units: u32,
    pub splash_kills_buildings: u32,
    pub garrison_kills_units: u32,
    pub killed_enemy_cost: u32,
    pub enemy_damage_dealt: f32,
    pub attacker_def_id: Option<String>,
    pub pulled_tile: Option<(u64, [f32; 3])>,
    pub power_drain_victim: Option<(u8, f32)>,
    pub siphon_victim_player: Option<u8>,
    pub steals_credits: u32,
    pub impact_tile: [i32; 2],
    pub impact_ref_y: f32,
    pub synced_civ_hps: Vec<(u64, f32)>,
}

pub(super) fn apply_post_impact_world_effects(
    world: &mut SimWorld,
    atk: &PendingAttack,
    loaded: &LoadedMod,
    ctx: PostImpactContext,
) {
    let mut enemy_damage_dealt = ctx.enemy_damage_dealt;
    let attacker_def_id = ctx.attacker_def_id;
    let w_def_opt = loaded.weapons.get(&atk.weapon_id);

    let bounty_ratio = world.player_cash_bounty_ratio(atk.attacker_player, atk.attacker_id, loaded);
    if let Some(att_econ) = world.players.get_mut(&atk.attacker_player) {
        let xp_gain = atk.damage * 0.25
            + if ctx.killed_unit || ctx.splash_kills_units > 0 || ctx.garrison_kills_units > 0 {
                35.0
            } else {
                0.0
            }
            + if ctx.killed_building || ctx.splash_kills_buildings > 0 {
                50.0
            } else {
                0.0
            };
        att_econ.add_commander_xp_with_rules(xp_gain, &loaded.manifest.rules);
        if bounty_ratio > 0.0 && ctx.killed_enemy_cost > 0 {
            let bounty = (ctx.killed_enemy_cost as f32 * bounty_ratio).round() as u32;
            att_econ.credits = att_econ.credits.saturating_add(bounty);
            att_econ
                .resources
                .insert("credits".to_string(), att_econ.credits);
        }
    }

    if let Some((pulled_id, new_pos)) = ctx.pulled_tile {
        let new_tile = world.grid.world_to_tile(new_pos);
        let is_water_drop = world.grid.water_tiles.contains(&new_tile)
            && !world.grid.bridge_tiles.contains(&new_tile);
        let mut passengers_to_sync = Vec::new();
        let mut fatal_water_drop = false;
        if let Some(t_ent) = world.entities.get_mut(&pulled_id) {
            t_ent.tile_origin = new_tile;
            passengers_to_sync = t_ent.passenger_ids.clone();
            if is_water_drop {
                let is_ground_only = match &t_ent.kind {
                    EntityKind::Unit { unit_id } => loaded
                        .units
                        .get(unit_id)
                        .map(|u| u.movement_domain == MovementDomain::Ground)
                        .unwrap_or(false),
                    _ => false,
                };
                if is_ground_only {
                    let prev_hp = t_ent.health;
                    t_ent.health = 0.0;
                    t_ent.script_ctx.health = 0.0;
                    if atk.attacker_player != 0 {
                        t_ent.last_damaged_by_player = atk.attacker_player;
                        t_ent.last_damaged_by_def_id = attacker_def_id.clone();
                    }
                    if t_ent.player_id != atk.attacker_player {
                        enemy_damage_dealt += prev_hp.max(0.0);
                    }
                    fatal_water_drop = true;
                }
            }
        }
        for pid in passengers_to_sync {
            if let Some(p_ent) = world.entities.get_mut(&pid) {
                p_ent.tile_origin = new_tile;
                p_ent.position = new_pos;
                if fatal_water_drop {
                    let prev_hp = p_ent.health;
                    p_ent.health = 0.0;
                    p_ent.script_ctx.health = 0.0;
                    if atk.attacker_player != 0 {
                        p_ent.last_damaged_by_player = atk.attacker_player;
                        p_ent.last_damaged_by_def_id = attacker_def_id.clone();
                    }
                    if p_ent.player_id != atk.attacker_player {
                        enemy_damage_dealt += prev_hp.max(0.0);
                    }
                }
            }
        }
        if !is_water_drop {
            let drop_y = world.grid.sample_world_height(new_pos[0], new_pos[2]);
            let crushed_ids: Vec<u64> = world
                .entities
                .values()
                .filter(|e| {
                    e.id != pulled_id
                        && e.health > 0.0
                        && e.transported_by.is_none()
                        && !e.is_invulnerable()
                        && !e.is_phased_out()
                        && e.parachute_timer <= 0.0
                        && !e.burrowed
                        && !e.is_airborne(loaded)
                        && (e.position[1] - drop_y).abs() <= 0.65
                        && {
                            let dx = e.position[0] - new_pos[0];
                            let dz = e.position[2] - new_pos[2];
                            e.tile_origin == new_tile || dx * dx + dz * dz <= 2.56
                        }
                        && match &e.kind {
                            EntityKind::Unit { unit_id } => loaded
                                .units
                                .get(unit_id)
                                .map(|u| {
                                    matches!(u.role, UnitRole::Infantry | UnitRole::Engineer)
                                        && !u.uncrushable
                                        && u.movement_domain == MovementDomain::Ground
                                })
                                .unwrap_or(false),
                            _ => false,
                        }
                })
                .map(|e| e.id)
                .collect();
            for cid in crushed_ids {
                if let Some(c_ent) = world.entities.get_mut(&cid) {
                    let prev_hp = c_ent.health;
                    c_ent.health = 0.0;
                    c_ent.script_ctx.health = 0.0;
                    if atk.attacker_player != 0 {
                        c_ent.last_damaged_by_player = atk.attacker_player;
                        c_ent.last_damaged_by_def_id = attacker_def_id.clone();
                    }
                    if c_ent.player_id != atk.attacker_player {
                        enemy_damage_dealt += prev_hp.max(0.0);
                    }
                }
            }
        }
    }

    if let Some((victim_pid, drain_secs)) = ctx.power_drain_victim {
        let cur = world
            .power_outage_timers
            .get(&victim_pid)
            .copied()
            .unwrap_or(0.0);
        world
            .power_outage_timers
            .insert(victim_pid, cur.max(drain_secs));
        if let Some(vp) = world.players.get_mut(&victim_pid) {
            vp.power_sabotage_timer = vp.power_sabotage_timer.max(drain_secs);
        }
        world.recalculate_power(loaded);
    }

    if let Some(victim_pid) = ctx.siphon_victim_player {
        let mut stolen = 0u32;
        if let Some(v_econ) = world.players.get_mut(&victim_pid) {
            stolen = v_econ.credits.min(ctx.steals_credits);
            v_econ.credits -= stolen;
            v_econ
                .resources
                .insert("credits".to_string(), v_econ.credits);
        }
        if stolen > 0 {
            if let Some(a_econ) = world.players.get_mut(&atk.attacker_player) {
                a_econ.credits = a_econ.credits.saturating_add(stolen);
                a_econ.credits_harvested = a_econ.credits_harvested.saturating_add(stolen);
                a_econ
                    .resources
                    .insert("credits".to_string(), a_econ.credits);
            }
        }
    }

    if let Some(zone) = w_def_opt.and_then(|w| w.on_hit_zone.as_ref()) {
        let cloud_radius = if zone.is_toxic_cloud && ctx.killed_unit {
            atk.splash_radius.max(zone.radius).max(3.8)
        } else {
            atk.splash_radius.max(zone.radius)
        };
        let cloud_dps = if zone.is_toxic_cloud && ctx.killed_unit {
            zone.dps.max(30.0)
        } else {
            zone.dps
        };
        world.radiation_zones.push(RadiationZone {
            position: atk.target_pos,
            radius: cloud_radius,
            dps: cloud_dps,
            remaining_secs: zone.duration_secs,
            is_toxic_cloud: zone.is_toxic_cloud,
            owner_player_id: atk.attacker_player,
        });
    }

    let impact_tile = ctx.impact_tile;
    let impact_ref_y = ctx.impact_ref_y;
    if !world.props.is_empty() {
        let prop_r = atk.splash_radius.max(1.6);
        let prop_r_sq = prop_r * prop_r;
        for prop in &mut world.props {
            let p_tile = world.grid.world_to_tile(prop.position);
            let plane_separated = (world.grid.bridge_tiles.contains(&impact_tile)
                || world.grid.bridge_tiles.contains(&p_tile))
                && (prop.position[1] - impact_ref_y).abs() > 0.65;
            if plane_separated {
                continue;
            }
            let dx = prop.position[0] - atk.target_pos[0];
            let dz = prop.position[2] - atk.target_pos[2];
            if dx * dx + dz * dz <= prop_r_sq {
                prop.health = (prop.health - atk.damage).max(0.0);
                if atk.attacker_player != 0 {
                    prop.last_damaged_by_player = atk.attacker_player;
                }
            }
        }
    }

    for (cid, next_hp) in ctx.synced_civ_hps {
        if let Some(cs) = world
            .civilian_structures
            .iter_mut()
            .find(|c| c.id == cid && !c.is_ruin)
        {
            cs.health = next_hp;
        }
    }

    ballistics::damage_civilian_structures_internal(
        world,
        atk.target_pos,
        atk.splash_radius,
        atk.damage,
        true,
        atk.attacker_player,
        attacker_def_id.as_deref(),
    );

    if atk.target_id == 0 && atk.damage >= 20.0 {
        let covering_bridge_entity = world.entities.values().find_map(|e| {
            if e.health <= 0.0 || e.is_invulnerable() {
                return None;
            }
            let EntityKind::Building { ref building_id } = e.kind else {
                return None;
            };
            let b_def = loaded.buildings.get(building_id)?;
            if b_def.role != BuildingRole::Bridge {
                return None;
            }
            let fp = e.custom_footprint.unwrap_or(b_def.footprint);
            if impact_tile[0] >= e.tile_origin[0]
                && impact_tile[0] < e.tile_origin[0] + fp[0] as i32
                && impact_tile[1] >= e.tile_origin[1]
                && impact_tile[1] < e.tile_origin[1] + fp[1] as i32
            {
                Some(e.id)
            } else {
                None
            }
        });
        if let Some(bridge_ent_id) = covering_bridge_entity {
            if let Some(b_ent) = world.entities.get_mut(&bridge_ent_id) {
                let prev_hp = b_ent.health;
                b_ent.health = (b_ent.health - atk.damage).max(0.0);
                b_ent.script_ctx.health = b_ent.health;
                if atk.attacker_player != 0 {
                    b_ent.last_damaged_by_player = atk.attacker_player;
                    b_ent.last_damaged_by_def_id = attacker_def_id.clone();
                }
                if b_ent.player_id != atk.attacker_player {
                    enemy_damage_dealt += (prev_hp - b_ent.health).max(0.0);
                }
            }
        } else if world.grid.bridge_tiles.contains(&impact_tile) {
            world.grid.set_footprint_bridge(impact_tile, [1, 1], false);
            if !world
                .destroyed_bridges
                .iter()
                .any(|b| b.tile_origin == impact_tile)
            {
                let bridge_b_id = loaded
                    .buildings
                    .values()
                    .find(|b| b.role == BuildingRole::Bridge)
                    .map(|b| b.id.clone())
                    .unwrap_or_else(|| "map_bridge_span".to_string());
                world
                    .destroyed_bridges
                    .push(crate::world::DestroyedBridgeSpan {
                        building_id: bridge_b_id,
                        tile_origin: impact_tile,
                        footprint: [1, 1],
                        rotated: false,
                        destroyed_by_player_id: atk.attacker_player,
                    });
                if atk.attacker_player != 0 {
                    if let Some(econ) = world.players.get_mut(&atk.attacker_player) {
                        econ.buildings_destroyed += 1;
                    }
                }
            }
            for e in world.entities.values_mut() {
                if e.health <= 0.0
                    || e.transported_by.is_some()
                    || e.parachute_timer > 0.0
                    || e.is_airborne(loaded)
                {
                    continue;
                }
                if let EntityKind::Unit { ref unit_id } = e.kind {
                    let et = world.grid.world_to_tile(e.position);
                    if et == impact_tile && world.grid.water_tiles.contains(&et) {
                        if let Some(u_def) = loaded.units.get(unit_id) {
                            let eff_domain = if e.script_ctx.deployed && u_def.lands_when_deployed {
                                MovementDomain::Ground
                            } else {
                                u_def.movement_domain
                            };
                            match eff_domain {
                                MovementDomain::Ground | MovementDomain::Chrono => {
                                    let prev_hp = e.health;
                                    e.health = 0.0;
                                    e.script_ctx.health = 0.0;
                                    if atk.attacker_player != 0 {
                                        e.last_damaged_by_player = atk.attacker_player;
                                        e.last_damaged_by_def_id = attacker_def_id.clone();
                                    }
                                    if e.player_id != atk.attacker_player {
                                        enemy_damage_dealt += prev_hp.max(0.0);
                                    }
                                }
                                MovementDomain::Naval | MovementDomain::Amphibious => {
                                    if !e.is_invulnerable() {
                                        let prev_hp = e.health;
                                        e.health = (e.health - 220.0).max(0.0);
                                        e.script_ctx.health = e.health;
                                        if atk.attacker_player != 0 {
                                            e.last_damaged_by_player = atk.attacker_player;
                                            e.last_damaged_by_def_id = attacker_def_id.clone();
                                        }
                                        if e.player_id != atk.attacker_player {
                                            enemy_damage_dealt += (prev_hp - e.health).max(0.0);
                                        }
                                    }
                                }
                                MovementDomain::Air | MovementDomain::Jumpjet => {}
                            }
                        }
                    }
                }
            }
        }
    }

    if enemy_damage_dealt > 0.0 {
        world.record_damage_dealt(
            atk.attacker_player,
            attacker_def_id.as_deref(),
            enemy_damage_dealt,
        );
    }

    // Invoke `ScriptContext` 2.0 `on_hit` / `on_damaged` hooks (`Task 232`)
    let mut on_hit_emitted: Vec<(u64, u8, Vec<rts_sdk::scripting::ScriptAction>)> = Vec::new();
    if atk.attacker_id != 0 {
        if let Some(att_hook) = world.entities.get(&atk.attacker_id).and_then(|e| {
            if e.health <= 0.0 {
                return None;
            }
            match &e.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .and_then(|u| u.script_hooks.get("on_hit"))
                    .cloned(),
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .and_then(|b| b.script_hooks.get("on_hit"))
                    .cloned(),
            }
        }) {
            if !att_hook.is_empty() && loaded.script_host.has_hook(&att_hook) {
                if let Some(att_mut) = world.entities.get_mut(&atk.attacker_id) {
                    let tid_opt = (atk.target_id != 0).then_some(atk.target_id);
                    SimWorld::sync_entity_script_ctx_pre_hook(
                        att_mut,
                        Some(atk.target_pos),
                        tid_opt,
                        Some(atk.attacker_id),
                    );
                    if loaded
                        .script_host
                        .invoke_hook(&att_hook, &mut att_mut.script_ctx)
                        .is_ok()
                    {
                        att_mut.health = att_mut.script_ctx.health.clamp(0.0, att_mut.max_health);
                        att_mut.script_ctx.health = att_mut.health;
                        if !att_mut.script_ctx.emitted_actions.is_empty() {
                            on_hit_emitted.push((
                                atk.attacker_id,
                                att_mut.player_id,
                                std::mem::take(&mut att_mut.script_ctx.emitted_actions),
                            ));
                        }
                    }
                }
            }
        }
    }
    if atk.target_id != 0 && atk.target_id != atk.attacker_id {
        if let Some(tgt_hook) = world
            .entities
            .get(&atk.target_id)
            .and_then(|e| match &e.kind {
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .and_then(|u| {
                        u.script_hooks
                            .get("on_damaged")
                            .or_else(|| u.script_hooks.get("on_hit"))
                    })
                    .cloned(),
                EntityKind::Building { building_id } => loaded
                    .buildings
                    .get(building_id)
                    .and_then(|b| {
                        b.script_hooks
                            .get("on_damaged")
                            .or_else(|| b.script_hooks.get("on_hit"))
                    })
                    .cloned(),
            })
        {
            if !tgt_hook.is_empty() && loaded.script_host.has_hook(&tgt_hook) {
                if let Some(tgt_mut) = world.entities.get_mut(&atk.target_id) {
                    let att_opt = (atk.attacker_id != 0).then_some(atk.attacker_id);
                    SimWorld::sync_entity_script_ctx_pre_hook(
                        tgt_mut,
                        Some(atk.target_pos),
                        Some(atk.target_id),
                        att_opt,
                    );
                    if loaded
                        .script_host
                        .invoke_hook(&tgt_hook, &mut tgt_mut.script_ctx)
                        .is_ok()
                    {
                        tgt_mut.health = tgt_mut.script_ctx.health.clamp(0.0, tgt_mut.max_health);
                        tgt_mut.script_ctx.health = tgt_mut.health;
                        if !tgt_mut.script_ctx.emitted_actions.is_empty() {
                            on_hit_emitted.push((
                                atk.target_id,
                                tgt_mut.player_id,
                                std::mem::take(&mut tgt_mut.script_ctx.emitted_actions),
                            ));
                        }
                    }
                }
            }
        }
    }
    for (src_id, src_pid, actions) in on_hit_emitted {
        world.execute_emitted_script_actions(Some(src_id), src_pid, actions, loaded);
    }

    let xp_gain = atk.damage + if ctx.target_killed { 75.0 } else { 0.0 };
    let mut xp_recipients: Vec<u64> = vec![atk.attacker_id];
    if let Some(att) = world.entities.get(&atk.attacker_id) {
        if let Some(cid) = att.transported_by {
            xp_recipients.push(cid);
        }
        let share_with_passengers = match &att.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| u.container.adapts_weapon_to_passenger || u.container.passengers_can_fire)
                .unwrap_or(false),
            EntityKind::Building { .. } => true,
        };
        if share_with_passengers && !att.passenger_ids.is_empty() {
            xp_recipients.extend(att.passenger_ids.iter().copied());
        }
    }
    for rec_id in xp_recipients {
        if let Some(rec) = world.entities.get_mut(&rec_id) {
            rec.experience += xp_gain;
            rec.veterancy =
                VeterancyRank::from_xp_with_rules(rec.experience, &loaded.manifest.rules);
        }
    }
}
