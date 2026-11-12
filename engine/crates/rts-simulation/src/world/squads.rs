use crate::combat::ballistics::civilian_ruin_cover_at;
use crate::command::GameCommand;
use crate::det_math::{det_distance_xz, det_sqrt, SimScalar};
use crate::world::economy::{can_afford_costs, deduct_costs, sync_player_ledgers};
use crate::world::{EntityKind, SimEntity, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, MovementDomain, UnitDef, UnitRole};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 3-tier tactical infantry/squad cover state (`Task 215`).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CoverLevel {
    #[default]
    None,
    Light,
    Heavy,
    Garrison,
}

impl CoverLevel {
    pub fn default_damage_multiplier(self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::Light => 0.75,
            Self::Heavy => 0.45,
            Self::Garrison => 0.35,
        }
    }
}

/// Resolves the absolute suppression threshold (`0..=max_suppression`) at which a squad becomes `is_pinned`.
pub fn squad_pin_suppression_value(u_def: &UnitDef) -> f32 {
    if let Some(ref sq) = u_def.squad_trait {
        let max_s = sq.max_suppression.max(10.0);
        let raw_t = sq.pin_suppression_threshold.unwrap_or(0.85);
        if raw_t <= 1.0 {
            raw_t.clamp(0.10, 1.0) * max_s
        } else {
            raw_t.clamp(1.0, max_s)
        }
    } else {
        85.0
    }
}

/// Evaluates the ambient 3-tier `CoverLevel` (`None`, `Light`, `Heavy`, `Garrison`) for an infantry/squad unit.
pub fn compute_unit_cover_level(
    world: &SimWorld,
    ent: &SimEntity,
    loaded: &LoadedMod,
) -> CoverLevel {
    let EntityKind::Unit { ref unit_id } = ent.kind else {
        return CoverLevel::None;
    };
    let Some(u_def) = loaded.units.get(unit_id) else {
        return CoverLevel::None;
    };
    let is_inf_or_squad = u_def.squad_trait.is_some()
        || matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer);
    if !is_inf_or_squad {
        return CoverLevel::None;
    }
    if ent.transported_by.is_some() {
        return CoverLevel::Garrison;
    }
    if u_def.movement_domain != MovementDomain::Ground || ent.is_airborne(loaded) {
        return CoverLevel::None;
    }

    let tx = ent.tile_origin[0];
    let tz = ent.tile_origin[1];

    // Heavy cover: collapsed civilian ruins, adjacent walls/sandbags/gates, or vehicle wrecks
    if civilian_ruin_cover_at(world, ent.tile_origin) > 0.0 {
        return CoverLevel::Heavy;
    }
    for dz in -1..=1 {
        for dx in -1..=1 {
            let nt = [tx + dx, tz + dz];
            if world.grid.wall_tiles.contains(&nt) || world.grid.gate_tiles.contains_key(&nt) {
                return CoverLevel::Heavy;
            }
        }
    }
    for scar in &world.battlefield_scars {
        if scar.is_wreck
            && det_distance_xz(ent.position, scar.position)
                <= SimScalar::from_f32(scar.radius.max(2.5))
        {
            return CoverLevel::Heavy;
        }
    }

    // Light cover: adjacent living trees/props or shell craters
    for p in &world.props {
        if p.health > 0.0 && (p.tile[0] - tx).abs() <= 1 && (p.tile[1] - tz).abs() <= 1 {
            return CoverLevel::Light;
        }
    }
    for scar in &world.battlefield_scars {
        if !scar.is_wreck
            && det_distance_xz(ent.position, scar.position)
                <= SimScalar::from_f32(scar.radius.max(2.8))
        {
            return CoverLevel::Light;
        }
    }

    CoverLevel::None
}

/// Scales a squad unit's weapon damage proportionally to its surviving squad members
/// (`squad_members_alive / member_count`, clamped to `[0.20, 1.0]`).
pub fn squad_firepower_multiplier(ent: &SimEntity, loaded: &LoadedMod) -> f32 {
    let EntityKind::Unit { ref unit_id } = ent.kind else {
        return 1.0;
    };
    let Some(u_def) = loaded.units.get(unit_id) else {
        return 1.0;
    };
    let Some(ref sq) = u_def.squad_trait else {
        return 1.0;
    };
    let total = sq.member_count.max(1) as f32;
    let alive = ent.squad_members_alive.clamp(1, sq.member_count.max(1)) as f32;
    let member_scale = (alive / total).clamp(0.20, 1.0);
    let extra_slot_bonus = if ent.squad_weapon_upgrades.len() > 1 {
        1.0 + 0.12 * ((ent.squad_weapon_upgrades.len() - 1) as f32)
    } else {
        1.0
    };
    member_scale * extra_slot_bonus
}

/// Computes the weapon cooldown multiplier (`>= 1.0`) caused by suppression morale penalty.
pub fn squad_suppression_cooldown_multiplier(ent: &SimEntity, loaded: &LoadedMod) -> f32 {
    if ent.suppression <= 0.0 {
        return 1.0;
    }
    let EntityKind::Unit { ref unit_id } = ent.kind else {
        return 1.0;
    };
    let Some(u_def) = loaded.units.get(unit_id) else {
        return 1.0;
    };
    let eff = if let Some(ref sq) = u_def.squad_trait {
        sq.suppression_efficiency(ent.suppression)
    } else if matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer) {
        let ratio = (ent.suppression / 100.0).clamp(0.0, 1.0);
        (1.0 - 0.45 * ratio).clamp(0.55, 1.0)
    } else {
        1.0
    };
    (1.0 / eff.max(0.35)).clamp(1.0, 2.85)
}

/// Evaluates ambient 3-tier `CoverLevel`, rubble-ruin cover, and directional wall/obstacle cover
/// for ground infantry & squads. Returns `(damage_multiplier, suppression_multiplier)`.
pub fn directional_and_ruin_cover_multipliers(
    world: &SimWorld,
    loaded: &LoadedMod,
    attacker_pos: [f32; 3],
    target: &SimEntity,
) -> (f32, f32) {
    let EntityKind::Unit { ref unit_id } = target.kind else {
        return (1.0, 1.0);
    };
    let Some(u_def) = loaded.units.get(unit_id) else {
        return (1.0, 1.0);
    };
    let is_inf_or_squad = u_def.squad_trait.is_some()
        || matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer);
    if !is_inf_or_squad
        || u_def.movement_domain != MovementDomain::Ground
        || target.is_airborne(loaded)
    {
        return (1.0, 1.0);
    }

    let mut dmg_mult = 1.0_f32;
    let mut supp_mult = 1.0_f32;

    // 0. 3-tier ambient tactical cover for `SquadTraitSpec` squads (`Light` = 0.75x default, `Heavy` = 0.45x default, `Garrison` = 0.35x)
    if let Some(ref sq) = u_def.squad_trait {
        let live_cover = if target.cover_level != CoverLevel::None || world.tick > 0 {
            target.cover_level
        } else {
            compute_unit_cover_level(world, target, loaded)
        };
        match live_cover {
            CoverLevel::None => {}
            CoverLevel::Light => {
                let m = sq.light_cover_damage_mult.unwrap_or(0.75).clamp(0.15, 1.0);
                dmg_mult = dmg_mult.min(m);
                supp_mult = supp_mult.min((m * 0.85).clamp(0.20, 1.0));
            }
            CoverLevel::Heavy => {
                let m = sq.heavy_cover_damage_mult.unwrap_or(0.45).clamp(0.15, 1.0);
                dmg_mult = dmg_mult.min(m);
                supp_mult = supp_mult.min((m * 0.80).clamp(0.20, 1.0));
            }
            CoverLevel::Garrison => {
                dmg_mult = dmg_mult.min(0.35);
                supp_mult = supp_mult.min(0.25);
            }
        }
    }

    // 1. Collapsed civilian rubble ruin cover (`cover_bonus` = damage reduction fraction)
    let ruin_bonus = civilian_ruin_cover_at(world, target.tile_origin);
    if ruin_bonus > 0.0 {
        dmg_mult = dmg_mult.min((1.0 - ruin_bonus).clamp(0.25, 1.0));
        supp_mult = supp_mult.min((1.0 - ruin_bonus * 1.15).clamp(0.20, 1.0));
    }

    // 2. Directional cover: check the adjacent tile toward `attacker_pos` for walls, obstacles, or structures
    let dx = SimScalar::from_f32(attacker_pos[0]) - SimScalar::from_f32(target.position[0]);
    let dz = SimScalar::from_f32(attacker_pos[2]) - SimScalar::from_f32(target.position[2]);
    let dist = det_sqrt(dx * dx + dz * dz);
    if dist > SimScalar::from_f32(1.5) {
        let dir_x = (dx / dist).to_f32();
        let dir_z = (dz / dist).to_f32();
        let step_tx = dir_x.round() as i32;
        let step_tz = dir_z.round() as i32;
        if step_tx != 0 || step_tz != 0 {
            let cover_tile = [
                target.tile_origin[0] + step_tx,
                target.tile_origin[1] + step_tz,
            ];
            let has_directional_cover = world.grid.blocked_tiles.contains(&cover_tile)
                || world.grid.gate_tiles.contains_key(&cover_tile)
                || world
                    .props
                    .iter()
                    .any(|p| p.health > 0.0 && p.tile == cover_tile);
            if has_directional_cover {
                let dir_mult = u_def
                    .squad_trait
                    .as_ref()
                    .map(|s| s.directional_cover_multiplier.clamp(0.20, 1.0))
                    .unwrap_or(0.75);
                dmg_mult = dmg_mult.min(dir_mult);
                supp_mult = supp_mult.min((dir_mult * 0.85).clamp(0.20, 1.0));
            }
        }
    }

    (dmg_mult, supp_mult)
}

/// Applies weapon suppression morale buildup and updates `squad_members_alive` casualties after damage.
pub fn apply_hit_suppression_and_casualties(
    target: &mut SimEntity,
    weapon_suppression: f32,
    supp_cover_mult: f32,
    loaded: &LoadedMod,
) {
    let EntityKind::Unit { ref unit_id } = target.kind else {
        return;
    };
    let Some(u_def) = loaded.units.get(unit_id) else {
        return;
    };

    if let Some(ref sq) = u_def.squad_trait {
        let max_m = sq.member_count.max(1);
        if target.health <= 0.0 {
            target.squad_members_alive = 0;
            target.is_pinned = false;
            return;
        }
        let ratio = (SimScalar::from_f32(target.health)
            / SimScalar::from_f32(target.max_health.max(1.0)))
        .to_f32()
        .clamp(0.0, 1.0);
        let alive = ((ratio * max_m as f32).ceil() as u8).clamp(1, max_m);
        target.squad_members_alive = alive;

        // Retreating squads have full suppression & pin immunity until reaching HQ/Barracks
        if target.is_retreating {
            target.is_pinned = false;
            return;
        }

        if weapon_suppression > 0.0 {
            let max_s = sq.max_suppression.max(10.0);
            let added = weapon_suppression * supp_cover_mult;
            target.suppression = (SimScalar::from_f32(target.suppression)
                + SimScalar::from_f32(added))
            .min(SimScalar::from_f32(max_s))
            .to_f32();
            if target.suppression >= squad_pin_suppression_value(u_def) {
                target.is_pinned = true;
            }
        }
    } else if matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer)
        && weapon_suppression > 0.0
        && target.health > 0.0
        && !target.is_retreating
    {
        let added = weapon_suppression * supp_cover_mult * 0.65;
        target.suppression = (SimScalar::from_f32(target.suppression) + SimScalar::from_f32(added))
            .min(SimScalar::from_f32(100.0))
            .to_f32();
    }
}

fn collect_reinforce_hubs(world: &SimWorld, loaded: &LoadedMod) -> Vec<(u8, [f32; 3], [i32; 2])> {
    let mut reinforce_hubs = Vec::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0 || ent.player_id == 0 || ent.transported_by.is_some() {
            continue;
        }
        match &ent.kind {
            EntityKind::Building { building_id } => {
                if ent.construction_progress >= 1.0
                    && !ent.power_disabled
                    && !ent.is_disabled_by_status()
                    && loaded.buildings.get(building_id).is_some_and(|b| {
                        matches!(
                            b.role,
                            BuildingRole::Barracks | BuildingRole::ConstructionYard
                        )
                    })
                {
                    reinforce_hubs.push((ent.player_id, ent.position, ent.tile_origin));
                }
            }
            EntityKind::Unit { unit_id } => {
                if !ent.is_disabled_by_status()
                    && loaded
                        .units
                        .get(unit_id)
                        .is_some_and(|u| u.container.capacity > 0)
                {
                    reinforce_hubs.push((ent.player_id, ent.position, ent.tile_origin));
                }
            }
        }
    }
    reinforce_hubs
}

fn compute_single_member_costs(
    u_def: &UnitDef,
    max_m: u8,
    ratio_cfg: f32,
) -> BTreeMap<String, u32> {
    let full_costs = u_def.effective_resource_costs();
    let mut member_costs = BTreeMap::new();
    let ratio = (ratio_cfg.clamp(0.0, 2.0)) / (max_m.max(1) as f32);
    for (res_id, &amt) in &full_costs {
        let c = ((amt as f32) * ratio).round() as u32;
        if c > 0 {
            member_costs.insert(res_id.clone(), c);
        }
    }
    member_costs
}

/// Builds a dense $O(1)$ per-tile cover lookup table (`0` = None, `1` = Light, `2` = Heavy)
/// for fast per-tick squad cover updates at 1,200+ entities.
fn build_tile_cover_grid(world: &SimWorld) -> Vec<u8> {
    let w = world.grid.width.max(1);
    let h = world.grid.height.max(1);
    let mut grid = vec![0_u8; (w * h) as usize];
    let mut mark = |tx: i32, tz: i32, level: u8| {
        if tx >= 0 && tx < w && tz >= 0 && tz < h {
            let idx = (tz * w + tx) as usize;
            if level > grid[idx] {
                grid[idx] = level;
            }
        }
    };

    for p in &world.props {
        if p.health > 0.0 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    mark(p.tile[0] + dx, p.tile[1] + dz, 1);
                }
            }
        }
    }
    for cs in &world.civilian_structures {
        if cs.is_ruin && cs.cover_bonus > 0.0 {
            for dz in 0..cs.footprint[1] as i32 {
                for dx in 0..cs.footprint[0] as i32 {
                    mark(cs.tile_pos[0] + dx, cs.tile_pos[1] + dz, 2);
                }
            }
        }
    }
    for &wt in &world.grid.wall_tiles {
        for dz in -1..=1 {
            for dx in -1..=1 {
                mark(wt[0] + dx, wt[1] + dz, 2);
            }
        }
    }
    for &gt in world.grid.gate_tiles.keys() {
        for dz in -1..=1 {
            for dx in -1..=1 {
                mark(gt[0] + dx, gt[1] + dz, 2);
            }
        }
    }
    let ts = world.grid.tile_size.max(0.5);
    for scar in &world.battlefield_scars {
        let lvl = if scar.is_wreck { 2 } else { 1 };
        let eff_r = if scar.is_wreck {
            scar.radius.max(2.5)
        } else {
            scar.radius.max(2.8)
        };
        let eff_r_s = SimScalar::from_f32(eff_r);
        let center_t = world.grid.world_to_tile(scar.position);
        let span = ((eff_r / ts).ceil() as i32).clamp(1, 4);
        for dz in -span..=span {
            for dx in -span..=span {
                let nt = [center_t[0] + dx, center_t[1] + dz];
                if nt[0] >= 0 && nt[0] < w && nt[1] >= 0 && nt[1] < h {
                    let wp = world.grid.tile_to_world(nt);
                    if det_distance_xz(wp, scar.position) <= eff_r_s {
                        mark(nt[0], nt[1], lvl);
                    }
                }
            }
        }
    }
    grid
}

/// Steps 3-tier cover state, pinning hysteresis, suppression morale recovery every tick, and
/// reinforces depleted `SquadTraitSpec` infantry squads near friendly `Barracks`/`HQ` or APCs once per second.
pub fn step_squad_morale_and_reinforcement(world: &mut SimWorld, loaded: &LoadedMod) {
    let mut has_any_squad = false;
    let mut has_any_suppressed = false;
    for e in world.entities.values() {
        if e.health <= 0.0 {
            continue;
        }
        if e.suppression > 0.0 || e.is_retreating || e.is_pinned {
            has_any_suppressed = true;
        }
        if !has_any_squad {
            if let EntityKind::Unit { ref unit_id } = e.kind {
                if loaded
                    .units
                    .get(unit_id)
                    .is_some_and(|u| u.squad_trait.is_some())
                {
                    has_any_squad = true;
                }
            }
        }
        if has_any_squad && has_any_suppressed {
            break;
        }
    }
    if !has_any_squad && !has_any_suppressed {
        return;
    }

    let dt_s = SimScalar::from_f32(world.dt);
    let reinforce_hubs = collect_reinforce_hubs(world, loaded);
    let cover_grid = if has_any_squad {
        build_tile_cover_grid(world)
    } else {
        Vec::new()
    };
    let grid_w = world.grid.width.max(1);
    let grid_h = world.grid.height.max(1);

    let unit_ids: Vec<u64> = world.entities.keys().copied().collect();
    for uid in &unit_ids {
        let Some((pos, tile, pid, supp, transported, airborne, unit_id)) =
            world.entities.get(uid).and_then(|e| {
                if e.health <= 0.0 {
                    return None;
                }
                match &e.kind {
                    EntityKind::Unit { unit_id } => Some((
                        e.position,
                        e.tile_origin,
                        e.player_id,
                        e.suppression,
                        e.transported_by.is_some(),
                        e.is_airborne(loaded),
                        unit_id.clone(),
                    )),
                    _ => None,
                }
            })
        else {
            continue;
        };
        let Some(u_def) = loaded.units.get(&unit_id) else {
            continue;
        };
        let is_squad = u_def.squad_trait.is_some();
        let is_inf = matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer);
        if !is_squad && !is_inf {
            continue;
        }

        let new_cover = if !is_squad {
            CoverLevel::None
        } else if transported {
            CoverLevel::Garrison
        } else if u_def.movement_domain != MovementDomain::Ground || airborne {
            CoverLevel::None
        } else if tile[0] >= 0 && tile[0] < grid_w && tile[1] >= 0 && tile[1] < grid_h {
            match cover_grid[(tile[1] * grid_w + tile[0]) as usize] {
                2 => CoverLevel::Heavy,
                1 => CoverLevel::Light,
                _ => CoverLevel::None,
            }
        } else {
            CoverLevel::None
        };

        let near_hub = supp > 0.0
            && reinforce_hubs.iter().any(|&(h_pid, h_pos, _)| {
                h_pid == pid && det_distance_xz(pos, h_pos) <= SimScalar::from_f32(8.0)
            });

        if let Some(ent) = world.entities.get_mut(uid) {
            ent.cover_level = new_cover;
            if ent.suppression > 0.0 {
                let decay_rate = if ent.is_retreating {
                    32.0
                } else if matches!(new_cover, CoverLevel::Heavy | CoverLevel::Garrison) || near_hub
                {
                    16.0
                } else if new_cover == CoverLevel::Light {
                    11.0
                } else {
                    8.0
                };
                ent.suppression = (SimScalar::from_f32(ent.suppression)
                    - SimScalar::from_f32(decay_rate) * dt_s)
                    .max(SimScalar::ZERO)
                    .to_f32();
            }
            if ent.is_retreating {
                ent.is_pinned = false;
            } else if is_squad {
                let pin_val = squad_pin_suppression_value(u_def);
                if ent.suppression >= pin_val {
                    ent.is_pinned = true;
                } else if ent.suppression <= pin_val * 0.70 {
                    ent.is_pinned = false;
                }
            } else {
                ent.is_pinned = false;
            }
        }
    }

    // Reinforce depleted infantry squads near friendly HQ/Barracks or APCs once per second (20 ticks)
    if !world.tick.is_multiple_of(20) || reinforce_hubs.is_empty() {
        return;
    }

    sync_player_ledgers(world);

    for uid in unit_ids {
        let Some((pid, pos, unit_id, alive, hp, max_hp, auto_reinf)) =
            world.entities.get(&uid).and_then(|e| {
                if e.health <= 0.0 || e.transported_by.is_some() || e.is_disabled_by_status() {
                    return None;
                }
                match &e.kind {
                    EntityKind::Unit { unit_id } => Some((
                        e.player_id,
                        e.position,
                        unit_id.clone(),
                        e.squad_members_alive,
                        e.health,
                        e.max_health,
                        e.squad_auto_reinforce,
                    )),
                    _ => None,
                }
            })
        else {
            continue;
        };
        if !auto_reinf {
            continue;
        }
        let Some(u_def) = loaded.units.get(&unit_id) else {
            continue;
        };
        let Some(ref sq) = u_def.squad_trait else {
            continue;
        };
        let max_m = sq.member_count.max(1);
        if alive >= max_m && hp >= max_hp {
            continue;
        }
        let near_hub = reinforce_hubs.iter().any(|&(h_pid, h_pos, _)| {
            h_pid == pid && det_distance_xz(pos, h_pos) <= SimScalar::from_f32(8.0)
        });
        if !near_hub {
            continue;
        }

        let member_costs = compute_single_member_costs(u_def, max_m, sq.reinforce_cost_ratio);
        let paid = if let Some(econ) = world.players.get_mut(&pid) {
            if can_afford_costs(econ, &member_costs) {
                deduct_costs(econ, &member_costs);
                true
            } else {
                false
            }
        } else {
            false
        };

        if paid {
            let hp_per_member = max_hp / (max_m as f32);
            if let Some(ent) = world.entities.get_mut(&uid) {
                ent.squad_members_alive = (ent.squad_members_alive + 1).min(max_m);
                ent.health = (SimScalar::from_f32(ent.health) + SimScalar::from_f32(hp_per_member))
                    .min(SimScalar::from_f32(ent.max_health))
                    .to_f32();
                ent.script_ctx.health = ent.health;
            }
        }
    }
}

/// Applies tactical squad commands (`RetreatSquad`, `SetSquadAutoReinforce`, `ReinforceSquadNow`, `UpgradeSquadWeapon`).
pub fn apply_squad_command(world: &mut SimWorld, cmd: GameCommand, loaded: &LoadedMod) {
    match cmd {
        GameCommand::RetreatSquad {
            player_id,
            squad_ids,
        } => {
            // Find friendly HQ (`ConstructionYard`) or `Barracks` retreat anchors
            let mut retreat_anchors: Vec<([f32; 3], [i32; 2], [u32; 2])> = Vec::new();
            for ent in world.entities.values() {
                if ent.player_id != player_id
                    || ent.health <= 0.0
                    || ent.construction_progress < 1.0
                {
                    continue;
                }
                if let EntityKind::Building { ref building_id } = ent.kind {
                    if let Some(b_def) = loaded.buildings.get(building_id) {
                        if matches!(
                            b_def.role,
                            BuildingRole::ConstructionYard | BuildingRole::Barracks
                        ) {
                            let fp = ent.custom_footprint.unwrap_or(b_def.footprint);
                            retreat_anchors.push((ent.position, ent.tile_origin, fp));
                        }
                    }
                }
            }
            if retreat_anchors.is_empty() {
                return;
            }

            for sid in squad_ids {
                let Some((start_pos, start_tile, unit_id, clearance)) =
                    world.entities.get(&sid).and_then(|e| {
                        if e.player_id != player_id
                            || e.health <= 0.0
                            || e.transported_by.is_some()
                            || e.is_phased_out()
                        {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Unit { unit_id } => Some((
                                e.position,
                                e.tile_origin,
                                unit_id.clone(),
                                crate::world::locomotors::unit_clearance_tiles(e, loaded),
                            )),
                            _ => None,
                        }
                    })
                else {
                    continue;
                };
                let Some(u_def) = loaded.units.get(&unit_id) else {
                    continue;
                };
                if u_def.squad_trait.is_none()
                    && !matches!(u_def.role, UnitRole::Infantry | UnitRole::Engineer)
                {
                    continue;
                }

                // Pick nearest retreat anchor
                let Some(&(_, anchor_origin, anchor_fp)) =
                    retreat_anchors.iter().min_by(|(pa, _, _), (pb, _, _)| {
                        det_distance_xz(start_pos, *pa).cmp(&det_distance_xz(start_pos, *pb))
                    })
                else {
                    continue;
                };

                let allowed_gates = world.allowed_gate_owners(player_id);
                // Find nearest walkable perimeter tile around the HQ/Barracks footprint
                let mut goal_tile = None;
                let mut best_d = SimScalar::from_f32(1.0e9);
                for dz in -2..=(anchor_fp[1] as i32 + 1) {
                    for dx in -2..=(anchor_fp[0] as i32 + 1) {
                        let cand = [anchor_origin[0] + dx, anchor_origin[1] + dz];
                        if world.grid.is_tile_traversable_for_domain(
                            cand,
                            u_def.movement_domain,
                            &allowed_gates,
                        ) {
                            let wp = world.grid.tile_to_world(cand);
                            let d = det_distance_xz(start_pos, wp);
                            if d < best_d {
                                best_d = d;
                                goal_tile = Some(cand);
                            }
                        }
                    }
                }
                let Some(goal) = goal_tile else {
                    continue;
                };

                let path = world.grid.find_path_with_clearance(
                    start_tile,
                    goal,
                    u_def.movement_domain,
                    0.0,
                    &allowed_gates,
                    clearance,
                );
                if let Some(ent) = world.entities.get_mut(&sid) {
                    ent.is_retreating = true;
                    ent.is_pinned = false;
                    ent.reverse_moving = false;
                    ent.target_entity_id = None;
                    ent.infiltrate_target_id = None;
                    ent.repair_hut_target_id = None;
                    ent.attack_ground_pos = None;
                    ent.attack_move_dest = None;
                    ent.patrol_route = None;
                    ent.patrol_waypoints.clear();
                    ent.guard_target_id = None;
                    ent.command_queue.clear();
                    ent.path_waypoints = if path.is_empty() {
                        vec![world.grid.tile_to_world(goal)]
                    } else {
                        path
                    };
                }
            }
        }
        GameCommand::SetSquadAutoReinforce {
            player_id,
            squad_ids,
            enabled,
        } => {
            for sid in squad_ids {
                if let Some(ent) = world.entities.get_mut(&sid) {
                    if ent.player_id == player_id && ent.health > 0.0 {
                        ent.squad_auto_reinforce = enabled;
                    }
                }
            }
        }
        GameCommand::ReinforceSquadNow {
            player_id,
            squad_ids,
        } => {
            sync_player_ledgers(world);
            let reinforce_hubs = collect_reinforce_hubs(world, loaded);
            let player_owns_any_hub = reinforce_hubs
                .iter()
                .any(|&(h_pid, _, _)| h_pid == player_id);

            for sid in squad_ids {
                let Some((pos, unit_id, alive, hp, max_hp)) =
                    world.entities.get(&sid).and_then(|e| {
                        if e.player_id != player_id
                            || e.health <= 0.0
                            || e.transported_by.is_some()
                            || e.is_disabled_by_status()
                        {
                            return None;
                        }
                        match &e.kind {
                            EntityKind::Unit { unit_id } => Some((
                                e.position,
                                unit_id.clone(),
                                e.squad_members_alive,
                                e.health,
                                e.max_health,
                            )),
                            _ => None,
                        }
                    })
                else {
                    continue;
                };
                let Some(u_def) = loaded.units.get(&unit_id) else {
                    continue;
                };
                let Some(ref sq) = u_def.squad_trait else {
                    continue;
                };
                let max_m = sq.member_count.max(1);
                if alive >= max_m && hp >= max_hp {
                    continue;
                }
                // Require proximity to a friendly HQ/Barracks/APC if the player owns one on map
                if player_owns_any_hub {
                    let near_hub = reinforce_hubs.iter().any(|&(h_pid, h_pos, _)| {
                        h_pid == player_id
                            && det_distance_xz(pos, h_pos) <= SimScalar::from_f32(10.0)
                    });
                    if !near_hub {
                        continue;
                    }
                }

                let member_costs =
                    compute_single_member_costs(u_def, max_m, sq.reinforce_cost_ratio);
                let missing = max_m.saturating_sub(alive).max(1);
                let mut reinforced_count = 0_u8;
                for _ in 0..missing {
                    let paid = if let Some(econ) = world.players.get_mut(&player_id) {
                        if can_afford_costs(econ, &member_costs) {
                            deduct_costs(econ, &member_costs);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if paid {
                        reinforced_count += 1;
                    } else {
                        break;
                    }
                }

                if reinforced_count > 0 {
                    let hp_per_member = max_hp / (max_m as f32);
                    if let Some(ent) = world.entities.get_mut(&sid) {
                        ent.squad_members_alive = ent
                            .squad_members_alive
                            .saturating_add(reinforced_count)
                            .min(max_m);
                        ent.health = (SimScalar::from_f32(ent.health)
                            + SimScalar::from_f32(hp_per_member * (reinforced_count as f32)))
                        .min(SimScalar::from_f32(ent.max_health))
                        .to_f32();
                        ent.script_ctx.health = ent.health;
                    }
                }
            }
        }
        GameCommand::UpgradeSquadWeapon {
            player_id,
            squad_id,
            upgrade_id,
        } => {
            sync_player_ledgers(world);
            let Some((unit_id, used_slots)) = world.entities.get(&squad_id).and_then(|e| {
                if e.player_id != player_id
                    || e.health <= 0.0
                    || e.is_disabled_by_status()
                    || e.is_retreating
                {
                    return None;
                }
                match &e.kind {
                    EntityKind::Unit { unit_id } => {
                        Some((unit_id.clone(), e.squad_weapon_upgrades.len()))
                    }
                    _ => None,
                }
            }) else {
                return;
            };
            let Some(u_def) = loaded.units.get(&unit_id) else {
                return;
            };
            let Some(ref sq) = u_def.squad_trait else {
                return;
            };
            if used_slots >= sq.weapon_upgrade_slots as usize {
                return;
            }
            if !sq.available_weapon_upgrades.is_empty()
                && !sq.available_weapon_upgrades.contains(&upgrade_id)
            {
                return;
            }

            let resolved_weapon_id = if loaded.weapons.contains_key(&upgrade_id) {
                Some(upgrade_id.clone())
            } else {
                upgrade_id
                    .strip_suffix("_upgrade")
                    .filter(|w| loaded.weapons.contains_key(*w))
                    .map(|w| w.to_string())
            };

            let upg_opt = loaded.upgrades.get(&upgrade_id).cloned();
            if upg_opt.is_none() && resolved_weapon_id.is_none() {
                return;
            }

            if let Some(ref upg) = upg_opt {
                if !world.has_prerequisites(player_id, &upg.prerequisites) {
                    return;
                }
                let mut costs = BTreeMap::new();
                if upg.cost > 0 {
                    costs.insert("credits".to_string(), upg.cost);
                }
                let paid = if let Some(econ) = world.players.get_mut(&player_id) {
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
                    return;
                }
            }

            if let Some(ent) = world.entities.get_mut(&squad_id) {
                ent.squad_weapon_upgrades.push(upgrade_id);
                if let Some(wid) = resolved_weapon_id {
                    ent.primary_weapon = Some(wid);
                }
                if let Some(upg) = upg_opt {
                    if upg.damage_bonus_mult > 0.0 && (upg.damage_bonus_mult - 1.0).abs() > 1e-4 {
                        ent.script_ctx.damage_multiplier *= upg.damage_bonus_mult;
                    }
                    if upg.speed_bonus_mult > 0.0 && (upg.speed_bonus_mult - 1.0).abs() > 1e-4 {
                        ent.script_ctx.speed_multiplier *= upg.speed_bonus_mult;
                    }
                    if upg.range_bonus.abs() > 1e-4 {
                        ent.script_ctx.range_bonus += upg.range_bonus;
                    }
                }
            }
        }
        _ => {}
    }
}
