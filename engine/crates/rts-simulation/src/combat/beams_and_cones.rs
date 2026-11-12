//! Penetrating `DeliveryKind::Beam` and angular `DeliveryKind::Cone` weapon resolution (`Task 233c`).

use super::{
    ballistics::{self, StructureOcclusionMap},
    directional_armor_damage_multiplier, distance_to_target_entity, entity_domain,
    impact::apply_attack_impact_with_grid,
    CombatEvent, PendingAttack,
};
use crate::det_math::{det_atan2, det_sin_cos, det_sqrt_f32};
use crate::world::{EntityKind, SimWorld};
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{BuildingRole, DeliveryKind, WeaponDef};

/// Clips a 3D beam/cone ray from `muzzle_pos` to `end_pos` against blocking terrain cliffs and
/// opaque walls/structures (`collides_with_terrain`, `collides_with_structures`).
/// Returns `(clipped_end_pos, hit_blocking_structure_id)`.
#[allow(clippy::too_many_arguments)]
fn clip_ray_against_terrain_and_structures(
    world: &SimWorld,
    attacker_id: u64,
    attacker_player: u8,
    muzzle_pos: [f32; 3],
    end_pos: [f32; 3],
    collides_with_terrain: bool,
    collides_with_structures: bool,
    structure_map: &StructureOcclusionMap,
) -> ([f32; 3], Option<u64>) {
    let dx = end_pos[0] - muzzle_pos[0];
    let dy = end_pos[1] - muzzle_pos[1];
    let dz = end_pos[2] - muzzle_pos[2];
    let len = det_sqrt_f32(dx * dx + dy * dy + dz * dz);
    if len <= 0.25 {
        return (end_pos, None);
    }
    let start_tile = world.grid.world_to_tile(muzzle_pos);
    let end_tile = world.grid.world_to_tile(end_pos);
    let start_elev = world
        .grid
        .sample_world_height(muzzle_pos[0], muzzle_pos[2])
        .max(world.grid.tile_elevation(start_tile));
    let steps = ((len / 0.45).ceil() as usize).clamp(1, 96);

    for s in 1..=steps {
        let t = (s as f32) / (steps as f32);
        let px = muzzle_pos[0] + dx * t;
        let py = muzzle_pos[1] + dy * t;
        let pz = muzzle_pos[2] + dz * t;
        let tile = world.grid.world_to_tile([px, py, pz]);
        if tile == start_tile {
            continue;
        }
        if collides_with_terrain && tile != end_tile {
            let terr_y = world
                .grid
                .sample_world_height(px, pz)
                .max(world.grid.tile_elevation(tile));
            if py <= terr_y + 0.05 || (terr_y > start_elev + 0.55 && py <= terr_y + 0.45) {
                return ([px, terr_y + 0.25, pz], None);
            }
        }
        if let Some(&(b_id, b_owner, b_role, roof_y)) = structure_map.get(&tile) {
            if b_id != attacker_id {
                let is_wall = matches!(b_role, BuildingRole::Wall | BuildingRole::Gate);
                let is_friendly = b_owner != 0 && !world.are_hostile(attacker_player, b_owner);
                let blocks = if is_wall {
                    !is_friendly || collides_with_structures
                } else {
                    collides_with_structures && !is_friendly
                };
                if blocks && py < roof_y {
                    return ([px, py, pz], (!is_friendly).then_some(b_id));
                }
            }
        }
    }
    (end_pos, None)
}

/// Shortest 2D XZ distance from `point` to segment `seg_a -> seg_b`, plus clamped parameter `t in [0, 1]`.
#[inline]
fn point_to_segment_xz(point: [f32; 3], seg_a: [f32; 3], seg_b: [f32; 3]) -> (f32, f32) {
    let ab_x = seg_b[0] - seg_a[0];
    let ab_z = seg_b[2] - seg_a[2];
    let len_sq = ab_x * ab_x + ab_z * ab_z;
    if len_sq <= 1e-6 {
        let dx = point[0] - seg_a[0];
        let dz = point[2] - seg_a[2];
        return (det_sqrt_f32(dx * dx + dz * dz), 0.0);
    }
    let ap_x = point[0] - seg_a[0];
    let ap_z = point[2] - seg_a[2];
    let t = ((ap_x * ab_x + ap_z * ab_z) / len_sq).clamp(0.0, 1.0);
    let cx = seg_a[0] + ab_x * t;
    let cz = seg_a[2] + ab_z * t;
    let dx = point[0] - cx;
    let dz = point[2] - cz;
    (det_sqrt_f32(dx * dx + dz * dz), t)
}

/// Computes per-candidate effective weapon damage preserving attacker multipliers relative to the
/// primary `atk.damage` while re-evaluating target-specific `ArmorTag`x`WarheadTag` and directional armor.
fn compute_secondary_hit_damage(
    world: &SimWorld,
    loaded: &LoadedMod,
    atk: &PendingAttack,
    w_def: &WeaponDef,
    target_id: u64,
) -> f32 {
    let Some(t_ent) = world.entities.get(&target_id) else {
        return atk.damage;
    };
    let attacker_scale = world
        .entities
        .get(&atk.attacker_id)
        .map(|att| {
            let (_, buff_dmg, _, _, _) = SimWorld::entity_buff_multipliers(att);
            let (upg_dmg, _, _, _) =
                world.player_entity_upgrade_multipliers(atk.attacker_player, att, loaded);
            let squad_dmg = crate::world::squads::squad_firepower_multiplier(att, loaded);
            let charge = if att.has_power_charge_boost() {
                1.35
            } else {
                1.0
            };
            att.script_ctx.damage_multiplier
                * att.veterancy.damage_bonus()
                * charge
                * buff_dmg
                * upg_dmg
                * squad_dmg
                * (w_def.burst.max(1) as f32)
        })
        .unwrap_or_else(|| (atk.damage / w_def.damage.max(1.0)).max(0.25));

    let base_vs_target = world.weapon_base_damage_vs_entity(w_def, t_ent, loaded);
    let dir_mult = directional_armor_damage_multiplier(atk.attacker_pos, t_ent, loaded);
    let (_, t_upg_arm, _, _) =
        world.player_entity_upgrade_multipliers(t_ent.player_id, t_ent, loaded);
    let (_, _, t_buff_arm, _, _) = SimWorld::entity_buff_multipliers(t_ent);
    let t_armor_mult = (t_ent.script_ctx.armor_multiplier
        * t_ent.veterancy.armor_bonus()
        * t_upg_arm
        * t_buff_arm)
        .max(0.1);
    (base_vs_target * attacker_scale * dir_mult) / t_armor_mult
}

/// Resolves `DeliveryKind::Beam` (penetrating line corridor) and `DeliveryKind::Cone`
/// (angular sector sweep), applying damage & `on_hit_status` to all intersected hostile targets.
pub(super) fn resolve_beam_or_cone_attack(
    world: &mut SimWorld,
    atk: &PendingAttack,
    loaded: &LoadedMod,
    spatial_opt: Option<&crate::pathfinding::WorldSpatialIndex>,
    structure_map: &StructureOcclusionMap,
) -> [f32; 3] {
    let Some(w_def) = loaded.weapons.get(&atk.weapon_id).cloned() else {
        apply_attack_impact_with_grid(world, atk, loaded, spatial_opt);
        return atk.target_pos;
    };
    let ballistics = w_def.resolved_ballistics();
    let muzzle_pos = [
        atk.attacker_pos[0],
        atk.attacker_pos[1] + 0.90,
        atk.attacker_pos[2],
    ];

    let dx_aim = atk.target_pos[0] - muzzle_pos[0];
    let dz_aim = atk.target_pos[2] - muzzle_pos[2];
    let aim_dist_xz = det_sqrt_f32(dx_aim * dx_aim + dz_aim * dz_aim);
    let (fwd_x, fwd_z) = if aim_dist_xz > 1e-4 {
        (dx_aim / aim_dist_xz, dz_aim / aim_dist_xz)
    } else {
        let fallback_yaw = world
            .entities
            .get(&atk.attacker_id)
            .map(|a| a.turret_facing_radians)
            .unwrap_or(0.0);
        det_sin_cos(fallback_yaw)
    };

    let eff_weap_range = world
        .entities
        .get(&atk.attacker_id)
        .map(|att| {
            let (_, _, _, upg_rng) =
                world.player_entity_upgrade_multipliers(att.attacker_player_or_self(), att, loaded);
            w_def.range * att.veterancy.range_bonus()
                + att.script_ctx.range_bonus
                + att.buff_range_bonus()
                + upg_rng
        })
        .unwrap_or(w_def.range)
        .max(aim_dist_xz.min(w_def.range + 2.5));

    if atk.delivery == DeliveryKind::Beam {
        if !ballistics.penetrating {
            apply_attack_impact_with_grid(world, atk, loaded, spatial_opt);
            return atk.target_pos;
        }
        let map_w = (world.grid.width as f32 * world.grid.tile_size).max(2.0);
        let map_h = (world.grid.height as f32 * world.grid.tile_size).max(2.0);
        let raw_end_x = (muzzle_pos[0] + fwd_x * eff_weap_range).clamp(0.2, map_w - 0.2);
        let raw_end_z = (muzzle_pos[2] + fwd_z * eff_weap_range).clamp(0.2, map_h - 0.2);
        let raw_end_y = world
            .grid
            .sample_world_height(raw_end_x, raw_end_z)
            .max(atk.target_pos[1])
            + 0.75;
        let (clipped_end, blocker_id) = clip_ray_against_terrain_and_structures(
            world,
            atk.attacker_id,
            atk.attacker_player,
            muzzle_pos,
            [raw_end_x, raw_end_y, raw_end_z],
            ballistics.collides_with_terrain,
            ballistics.collides_with_structures,
            structure_map,
        );

        let beam_corridor_half_w = w_def.splash_radius.max(0.85);
        let mid_pos = [
            0.5 * (muzzle_pos[0] + clipped_end[0]),
            0.5 * (muzzle_pos[1] + clipped_end[1]),
            0.5 * (muzzle_pos[2] + clipped_end[2]),
        ];
        let search_r = eff_weap_range * 0.5 + 5.0;

        let mut candidates: Vec<(i32, u64, [f32; 3])> = Vec::new();
        let mut consider = |eid: u64| {
            if eid == atk.attacker_id {
                return;
            }
            let Some(ent) = world.entities.get(&eid) else {
                return;
            };
            if ent.health <= 0.0
                || ent.transported_by.is_some()
                || ent.is_invulnerable()
                || (ent.is_phased_out() && w_def.temporal_erase_dps <= 0.0)
            {
                return;
            }
            if !(atk.friendly_fire || world.are_hostile(atk.attacker_player, ent.player_id)) {
                return;
            }
            if !w_def.can_target_domain(entity_domain(ent, loaded)) {
                return;
            }
            let entity_rad = match &ent.kind {
                EntityKind::Building { .. } => {
                    let fp = world.entity_footprint(ent, loaded);
                    (fp[0].max(fp[1]) as f32) * world.grid.tile_size * 0.55
                }
                EntityKind::Unit { unit_id } => loaded
                    .units
                    .get(unit_id)
                    .map(|u| (u.visual.scale[0].max(u.visual.scale[2]) * 0.5).clamp(0.45, 1.8))
                    .unwrap_or(0.55),
            };
            let (perp_d, t_param) = point_to_segment_xz(ent.position, muzzle_pos, clipped_end);
            if perp_d <= beam_corridor_half_w + entity_rad {
                candidates.push(((t_param * 10_000.0) as i32, eid, ent.position));
            } else if Some(eid) == blocker_id {
                candidates.push((10_000, eid, ent.position));
            }
        };

        if let Some(grid) = spatial_opt {
            grid.for_each_in_radius(mid_pos, search_r, |eid, _, _| consider(eid));
        } else {
            let ids: Vec<u64> = world.entities.keys().copied().collect();
            for eid in ids {
                consider(eid);
            }
        }
        candidates.sort_unstable_by_key(|&(ord, eid, _)| (ord, eid));
        candidates.dedup_by_key(|&mut (_, eid, _)| eid);

        if candidates.is_empty() {
            ballistics::damage_civilian_structures_internal(
                world,
                clipped_end,
                w_def.splash_radius.max(0.8),
                atk.damage,
                false,
                atk.attacker_player,
                None,
            );
        } else {
            for (idx, (_, hit_id, hit_pos)) in candidates.into_iter().enumerate() {
                let hit_dmg = if hit_id == atk.target_id {
                    atk.damage
                } else {
                    compute_secondary_hit_damage(world, loaded, atk, &w_def, hit_id)
                };
                let mut sub_atk = atk.clone();
                sub_atk.target_id = hit_id;
                sub_atk.target_pos = hit_pos;
                sub_atk.damage = hit_dmg;
                sub_atk.splash_radius = 0.0;
                apply_attack_impact_with_grid(world, &sub_atk, loaded, None);
                if idx > 0 {
                    world.combat_events.push(CombatEvent {
                        attacker_id: atk.attacker_id,
                        target_id: hit_id,
                        attacker_pos: atk.attacker_pos,
                        target_pos: hit_pos,
                        weapon_id: atk.weapon_id.clone(),
                        damage: hit_dmg,
                        projectile_style: atk.projectile_style,
                    });
                }
            }
        }
        return [clipped_end[0], atk.target_pos[1], clipped_end[2]];
    }

    // `DeliveryKind::Cone`: 2D/3D sector sweep within `eff_weap_range` and `cone_half_angle_deg`
    let half_angle_deg = ballistics
        .cone_half_angle_deg
        .max(w_def.cone_angle_deg * 0.5)
        .clamp(5.0, 89.0);
    let half_angle_rad = half_angle_deg.to_radians();
    let (_, cos_half) = det_sin_cos(half_angle_rad);
    let aim_yaw = det_atan2(fwd_x, fwd_z);

    let mut cone_hits: Vec<(u64, [f32; 3])> = Vec::new();
    let mut consider_cone = |eid: u64| {
        if eid == atk.attacker_id {
            return;
        }
        let Some(ent) = world.entities.get(&eid) else {
            return;
        };
        if ent.health <= 0.0
            || ent.transported_by.is_some()
            || ent.is_invulnerable()
            || (ent.is_phased_out() && w_def.temporal_erase_dps <= 0.0)
        {
            return;
        }
        if !(atk.friendly_fire || world.are_hostile(atk.attacker_player, ent.player_id)) {
            return;
        }
        if !w_def.can_target_domain(entity_domain(ent, loaded)) {
            return;
        }
        let edge_d = distance_to_target_entity(world, loaded, atk.attacker_pos, ent);
        if edge_d > eff_weap_range + 0.35 || edge_d < w_def.min_range {
            return;
        }
        let is_primary = eid == atk.target_id;
        if !is_primary && edge_d > 0.35 {
            let tx = ent.position[0] - atk.attacker_pos[0];
            let tz = ent.position[2] - atk.attacker_pos[2];
            let t_len = det_sqrt_f32(tx * tx + tz * tz);
            if t_len > 1e-4 {
                let dot = (tx * fwd_x + tz * fwd_z) / t_len;
                // Allow slight angular slack for large footprints at close range
                let slack_rad = (0.65 / t_len.max(1.0)).min(0.18);
                let cand_yaw = det_atan2(tx, tz);
                let mut diff = (cand_yaw - aim_yaw).abs();
                if diff > std::f32::consts::PI {
                    diff = std::f32::consts::TAU - diff;
                }
                if dot < cos_half && diff > half_angle_rad + slack_rad {
                    return;
                }
            }
        }
        if ballistics.collides_with_terrain || ballistics.collides_with_structures {
            let cand_aim = [ent.position[0], ent.position[1] + 0.75, ent.position[2]];
            let (clipped, blocker) = clip_ray_against_terrain_and_structures(
                world,
                atk.attacker_id,
                atk.attacker_player,
                muzzle_pos,
                cand_aim,
                ballistics.collides_with_terrain,
                ballistics.collides_with_structures,
                structure_map,
            );
            if blocker.is_some() && blocker != Some(eid) {
                return;
            }
            let rem_dx = cand_aim[0] - clipped[0];
            let rem_dz = cand_aim[2] - clipped[2];
            if rem_dx * rem_dx + rem_dz * rem_dz > 1.44 {
                return;
            }
        }
        cone_hits.push((eid, ent.position));
    };

    if let Some(grid) = spatial_opt {
        grid.for_each_in_radius(atk.attacker_pos, eff_weap_range + 5.0, |eid, _, _| {
            consider_cone(eid);
        });
    } else {
        let ids: Vec<u64> = world.entities.keys().copied().collect();
        for eid in ids {
            consider_cone(eid);
        }
    }
    cone_hits.sort_unstable_by_key(|&(eid, _)| eid);
    cone_hits.dedup_by_key(|&mut (eid, _)| eid);

    for (idx, (hit_id, hit_pos)) in cone_hits.into_iter().enumerate() {
        let hit_dmg = if hit_id == atk.target_id {
            atk.damage
        } else {
            compute_secondary_hit_damage(world, loaded, atk, &w_def, hit_id)
        };
        let mut sub_atk = atk.clone();
        sub_atk.target_id = hit_id;
        sub_atk.target_pos = hit_pos;
        sub_atk.damage = hit_dmg;
        sub_atk.splash_radius = 0.0;
        apply_attack_impact_with_grid(world, &sub_atk, loaded, None);
        if idx > 0 {
            world.combat_events.push(CombatEvent {
                attacker_id: atk.attacker_id,
                target_id: hit_id,
                attacker_pos: atk.attacker_pos,
                target_pos: hit_pos,
                weapon_id: atk.weapon_id.clone(),
                damage: hit_dmg,
                projectile_style: atk.projectile_style,
            });
        }
    }
    atk.target_pos
}

trait AttackerPlayerExt {
    fn attacker_player_or_self(&self) -> u8;
}

impl AttackerPlayerExt for crate::world::SimEntity {
    #[inline]
    fn attacker_player_or_self(&self) -> u8 {
        self.player_id
    }
}
