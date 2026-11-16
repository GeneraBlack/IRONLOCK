//! Allied Weather Control Device (`WeatherStormCell`) multi-bolt lightning storm cell (`6.0s`,
//! `10` deterministic staggered lightning bolts across `7.5m`, and `10.0s` hostile radar jam).

use serde::{Deserialize, Serialize};

use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ProjectileStyle, SuperweaponType, SupportPowerKind};

use crate::combat::{apply_emp_to_entity, CombatEvent};
use crate::command::GameCommand;
use crate::world::{DestructionEvent, EntityKind, SimBattlefieldScar, SimWorld, TacticalPing};

pub const WEATHER_STORM_DURATION_SECS: f32 = 6.0;
pub const WEATHER_STORM_RADIUS: f32 = 7.5;
pub const WEATHER_STORM_TOTAL_BOLTS: u8 = 10;
pub const WEATHER_STORM_RADAR_JAM_SECS: f32 = 10.0;
const BOLT_SPLASH_RADIUS: f32 = 3.2;

/// Active Weather Control Storm Cell hovering over a target zone (`SimWorld.weather_storms`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherStormCell {
    pub owner_player_id: u8,
    pub center: [f32; 3],
    pub radius: f32,
    pub duration_secs: f32,
    pub remaining_secs: f32,
    pub bolts_fired: u8,
    pub total_bolts: u8,
    pub damage_per_bolt: f32,
    pub seed: u64,
}

fn splitmix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Computes deterministic `(x, z)` strike coordinates for `bolt_index` (`0..10`) inside `radius`.
/// Bolt `0` strikes dead-center on target; bolts `1..9` spiral across `radius` deterministically.
pub fn deterministic_bolt_world_pos(
    world: &SimWorld,
    center: [f32; 3],
    radius: f32,
    seed: u64,
    bolt_index: u8,
) -> [f32; 3] {
    if bolt_index == 0 {
        let gy = world.grid.sample_world_height(center[0], center[2]);
        return [center[0], gy, center[2]];
    }
    let h = splitmix64(seed ^ ((bolt_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    let angle = ((bolt_index as f32) * 2.399_963_1) + (((h & 0xFFFF) as f32) / 65535.0) * 0.65;
    let ring_t = ((bolt_index as f32) / (WEATHER_STORM_TOTAL_BOLTS as f32)).clamp(0.18, 0.92);
    let jitter = ((((h >> 16) & 0xFFFF) as f32) / 65535.0) * 0.22;
    let dist = (ring_t * 0.78 + jitter).clamp(0.15, 0.96) * radius;
    let (sin_a, cos_a) = crate::det_math::det_sin_cos(angle);
    let map_w = (world.grid.width.max(4) as f32) * world.grid.tile_size;
    let map_h = (world.grid.height.max(4) as f32) * world.grid.tile_size;
    let bx = (center[0] + cos_a * dist).clamp(0.5, (map_w - 0.5).max(0.5));
    let bz = (center[2] + sin_a * dist).clamp(0.5, (map_h - 0.5).max(0.5));
    let by = world.grid.sample_world_height(bx, bz);
    [bx, by, bz]
}

impl SimWorld {
    /// Returns `true` when `player_id`'s tactical radar is jammed by an enemy Weather Storm.
    pub fn is_player_radar_jammed(&self, player_id: u8) -> bool {
        self.radar_jam_timers
            .get(&player_id)
            .copied()
            .unwrap_or(0.0)
            > 0.0
    }

    /// Spawns an active `WeatherStormCell`, jams hostile radar for `10.0s`, and strikes bolt #0.
    pub fn spawn_weather_storm_cell(
        &mut self,
        owner_player_id: u8,
        target_pos: [f32; 3],
        total_damage_hint: f32,
        radius_hint: f32,
        loaded: &LoadedMod,
    ) {
        let radius = if radius_hint > 0.0 {
            radius_hint.max(WEATHER_STORM_RADIUS)
        } else {
            WEATHER_STORM_RADIUS
        };
        let damage_per_bolt = if total_damage_hint > 0.0 {
            (total_damage_hint * 0.34).clamp(160.0, 320.0)
        } else {
            220.0
        };
        let seed = self.tick.wrapping_mul(0x517C_C1B7_2722_0A95)
            ^ ((owner_player_id as u64) << 48)
            ^ (target_pos[0].to_bits() as u64)
            ^ ((target_pos[2].to_bits() as u64) << 16);

        let hostile_pids: Vec<u8> = self
            .players
            .keys()
            .copied()
            .filter(|&pid| pid != 0 && self.are_hostile(owner_player_id, pid))
            .collect();
        for h_pid in hostile_pids {
            let cur = self.radar_jam_timers.get(&h_pid).copied().unwrap_or(0.0);
            self.radar_jam_timers
                .insert(h_pid, cur.max(WEATHER_STORM_RADAR_JAM_SECS));
        }

        self.last_event_world_pos = Some(target_pos);
        self.eva_transmissions.push((
            "EVA".to_string(),
            "Warning: Lightning Storm Created!".to_string(),
        ));
        self.active_pings.push(TacticalPing {
            player_id: owner_player_id,
            position: target_pos,
            remaining_secs: WEATHER_STORM_DURATION_SECS,
        });

        let mut cell = WeatherStormCell {
            owner_player_id,
            center: target_pos,
            radius,
            duration_secs: WEATHER_STORM_DURATION_SECS,
            remaining_secs: WEATHER_STORM_DURATION_SECS,
            bolts_fired: 0,
            total_bolts: WEATHER_STORM_TOTAL_BOLTS,
            damage_per_bolt,
            seed,
        };
        strike_single_storm_bolt(self, &mut cell, loaded);
        self.weather_storms.push(cell);
    }
}

/// Intercepts `CastSupportPower` when firing `SupportPowerKind::LightningStorm` so it spawns
/// a staggered 10-bolt `WeatherStormCell` instead of a single instant blast.
pub(crate) fn try_intercept_weather_storm_command(
    world: &mut SimWorld,
    cmd: &GameCommand,
    loaded: &LoadedMod,
) -> bool {
    let GameCommand::CastSupportPower {
        player_id,
        ref power_id,
        target_pos,
        ..
    } = *cmd
    else {
        return false;
    };

    let scripted_override = world
        .players
        .get(&player_id)
        .and_then(|e| e.support_power_cooldowns.get(power_id).copied())
        == Some(-1.0);
    let avail = world.available_support_powers_for_player(player_id, loaded);
    let Some((sp_def, rem_cd)) = avail
        .into_iter()
        .find(|(sp, _)| &sp.id == power_id)
        .or_else(|| {
            if scripted_override {
                loaded
                    .buildings
                    .values()
                    .flat_map(|b| b.support_powers.iter())
                    .find(|sp| &sp.id == power_id)
                    .cloned()
                    .map(|sp| (sp, 0.0))
            } else {
                None
            }
        })
    else {
        return false;
    };
    if sp_def.kind != SupportPowerKind::LightningStorm {
        return false;
    }
    if (rem_cd > 0.0 || !world.superweapons_enabled) && !scripted_override {
        return true;
    }
    let applied_cd = sp_def.cooldown_secs.max(1.0);
    if let Some(econ) = world.players.get_mut(&player_id) {
        econ.support_power_cooldowns
            .insert(power_id.clone(), applied_cd);
    }
    let provider_ids: Vec<u64> = world
        .entities
        .values()
        .filter(|e| {
            e.player_id == player_id
                && world.is_building_online(e)
                && match &e.kind {
                    EntityKind::Building { building_id } => loaded
                        .buildings
                        .get(building_id)
                        .is_some_and(|b| b.support_powers.iter().any(|sp| &sp.id == power_id)),
                    _ => false,
                }
        })
        .map(|e| e.id)
        .collect();
    for pid in provider_ids {
        if let Some(b_mut) = world.entities.get_mut(&pid) {
            b_mut.ability_cooldown = b_mut.ability_cooldown.max(applied_cd);
            b_mut.ability_max_cooldown = b_mut.ability_max_cooldown.max(applied_cd);
        }
    }
    world.spawn_weather_storm_cell(player_id, target_pos, sp_def.damage, sp_def.radius, loaded);
    true
}

impl SimWorld {
    /// Executes `WeatherStorm` as a multi-bolt storm cell, or delegates to `execute_unified_superweapon`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute_superweapon_with_weather_storm(
        &mut self,
        player_id: u8,
        source_id: Option<u64>,
        sw: SuperweaponType,
        sp_def: Option<&rts_sdk::schema::SupportPowerDef>,
        src_pos: Option<[f32; 3]>,
        target_pos: [f32; 3],
        loaded: &LoadedMod,
    ) {
        if sw == SuperweaponType::WeatherStorm {
            let dmg = sp_def.map(|s| s.damage).unwrap_or(650.0);
            let rad = sp_def.map(|s| s.radius).unwrap_or(7.5);
            self.spawn_weather_storm_cell(player_id, target_pos, dmg, rad, loaded);
        } else {
            self.execute_unified_superweapon(
                player_id, source_id, sw, sp_def, src_pos, target_pos, loaded,
            );
        }
    }
}

fn strike_single_storm_bolt(world: &mut SimWorld, cell: &mut WeatherStormCell, loaded: &LoadedMod) {
    if cell.bolts_fired >= cell.total_bolts {
        return;
    }
    let bolt_idx = cell.bolts_fired;
    cell.bolts_fired += 1;
    let bolt_pos =
        deterministic_bolt_world_pos(world, cell.center, cell.radius, cell.seed, bolt_idx);

    let r_sq = BOLT_SPLASH_RADIUS * BOLT_SPLASH_RADIUS;
    let ts = world.grid.tile_size.max(1.0);
    let mut hits: Vec<(u64, f32)> = Vec::new();
    for ent in world.entities.values() {
        if !ent.is_targetable()
            || ent.is_invulnerable()
            || !world.are_hostile(cell.owner_player_id, ent.player_id)
        {
            continue;
        }
        let fp = world.entity_footprint(ent, loaded);
        let half_w = (fp[0].max(1) as f32) * ts * 0.5;
        let half_h = (fp[1].max(1) as f32) * ts * 0.5;
        let dx = ((ent.position[0] - bolt_pos[0]).abs() - half_w).max(0.0);
        let dz = ((ent.position[2] - bolt_pos[2]).abs() - half_h).max(0.0);
        let dist_sq = dx * dx + dz * dz;
        if dist_sq <= r_sq {
            let dist = crate::det_math::det_sqrt_f32(dist_sq);
            let falloff = (1.0 - (dist / BOLT_SPLASH_RADIUS) * 0.40).clamp(0.50, 1.0);
            hits.push((ent.id, cell.damage_per_bolt * falloff));
        }
    }

    let mut total_dealt = 0.0_f32;
    for (eid, delta) in hits {
        if let Some(ent) = world.entities.get_mut(&eid) {
            let applied = delta.min(ent.health.max(0.0));
            ent.last_damaged_by_player = cell.owner_player_id;
            ent.health = (ent.health - delta).max(0.0);
            ent.script_ctx.health = ent.health;
            if ent.health > 0.0 {
                apply_emp_to_entity(ent, 2.5, loaded);
            }
            total_dealt += applied;
        }
    }
    if total_dealt > 0.0 {
        world.record_damage_dealt(
            cell.owner_player_id,
            Some("weather_lightning_storm"),
            total_dealt,
        );
        world.recalculate_power(loaded);
    }

    for prop in &mut world.props {
        let dx = prop.position[0] - bolt_pos[0];
        let dz = prop.position[2] - bolt_pos[2];
        if dx * dx + dz * dz <= r_sq {
            prop.last_damaged_by_player = cell.owner_player_id;
            prop.health = 0.0;
        }
    }

    let sky_origin = [bolt_pos[0], bolt_pos[1] + 18.0, bolt_pos[2]];
    world.combat_events.push(CombatEvent {
        attacker_id: 0,
        target_id: 0,
        attacker_pos: sky_origin,
        target_pos: bolt_pos,
        weapon_id: "weather_lightning_storm".to_string(),
        damage: cell.damage_per_bolt,
        projectile_style: ProjectileStyle::Tesla,
    });
    world.destruction_events.push(DestructionEvent {
        entity_id: 992_000 + world.tick * 16 + (bolt_idx as u64),
        player_id: cell.owner_player_id,
        position: bolt_pos,
        is_building: false,
        radius: BOLT_SPLASH_RADIUS,
        is_sold: false,
    });
    world.battlefield_scars.push(SimBattlefieldScar {
        position: bolt_pos,
        radius: 2.2,
        is_wreck: false,
        remaining_secs: 28.0,
    });
}

/// Ticks `radar_jam_timers` and advances all active `WeatherStormCell`s, firing staggered bolts.
pub(crate) fn step_weather_storms(world: &mut SimWorld, loaded: &LoadedMod) {
    let dt = world.dt;
    if !world.radar_jam_timers.is_empty() {
        for rem in world.radar_jam_timers.values_mut() {
            *rem = (*rem - dt).max(0.0);
        }
        world.radar_jam_timers.retain(|_, &mut rem| rem > 0.0);
    }
    if world.weather_storms.is_empty() {
        return;
    }

    let mut active_cells = std::mem::take(&mut world.weather_storms);
    let mut keep_cells = Vec::with_capacity(active_cells.len());

    for mut cell in active_cells.drain(..) {
        cell.remaining_secs -= dt;
        let elapsed = (cell.duration_secs - cell.remaining_secs).max(0.0);
        let interval = (cell.duration_secs / (cell.total_bolts.max(1) as f32)).max(0.1);
        let expected_bolts = ((elapsed / interval).floor() as u8)
            .saturating_add(1)
            .min(cell.total_bolts);
        while cell.bolts_fired < expected_bolts {
            strike_single_storm_bolt(world, &mut cell, loaded);
        }
        if cell.remaining_secs <= 0.0 {
            while cell.bolts_fired < cell.total_bolts {
                strike_single_storm_bolt(world, &mut cell, loaded);
            }
        } else {
            keep_cells.push(cell);
        }
    }
    world.weather_storms = keep_cells;
}

pub(crate) fn pre_combat_radiation_shields(
    world: &mut SimWorld,
    loaded: &LoadedMod,
) -> Vec<(u64, f32, f32)> {
    if world.radiation_zones.is_empty() {
        return Vec::new();
    }
    let resist_map: std::collections::BTreeMap<u8, f32> = world
        .players
        .keys()
        .filter_map(|&pid| {
            world
                .player_sub_faction_def(pid, loaded)
                .filter(|sf| sf.radiation_resist > 0.001)
                .map(|sf| (pid, sf.radiation_resist.clamp(0.0, 0.95)))
        })
        .collect();
    if resist_map.is_empty() {
        return Vec::new();
    }
    let dt = world.dt;
    let mut shields = Vec::new();
    for ent in world.entities.values_mut() {
        let Some(&resist) = resist_map.get(&ent.player_id) else {
            continue;
        };
        let is_ground = matches!(&ent.kind, EntityKind::Unit { unit_id }
            if loaded.units.get(unit_id).is_some_and(|u| !matches!(u.movement_domain, rts_sdk::schema::MovementDomain::Air | rts_sdk::schema::MovementDomain::Jumpjet)));
        if !is_ground || !ent.is_targetable() || ent.is_invulnerable() || ent.parachute_timer > 0.0
        {
            continue;
        }
        let rad_dmg: f32 = world
            .radiation_zones
            .iter()
            .filter(|z| z.remaining_secs > 0.0 && !z.is_toxic_cloud)
            .filter(|z| {
                let dx = ent.position[0] - z.position[0];
                let dz = ent.position[2] - z.position[2];
                dx * dx + dz * dz <= z.radius * z.radius
            })
            .map(|z| z.dps * dt)
            .sum();
        if rad_dmg > 0.0 {
            let orig_max = ent.max_health;
            let shield = rad_dmg * resist;
            ent.max_health += shield;
            ent.health += shield;
            ent.script_ctx.max_health = ent.max_health;
            ent.script_ctx.health = ent.health;
            shields.push((ent.id, orig_max, shield));
        }
    }
    shields
}

pub(crate) fn post_combat_radiation_shields(world: &mut SimWorld, shields: Vec<(u64, f32, f32)>) {
    for (eid, orig_max, _) in shields {
        if let Some(ent) = world.entities.get_mut(&eid) {
            ent.max_health = orig_max;
            ent.health = ent.health.min(orig_max);
            ent.script_ctx.max_health = ent.max_health;
            ent.script_ctx.health = ent.health;
        }
    }
}
