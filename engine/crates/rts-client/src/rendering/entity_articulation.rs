use crate::rendering::{
    build_unit_pbr_team_material, compute_articulated_part_pose, is_lod1_node_name,
    lod0_visibility_range, lod1_visibility_range, preview_stage, ArticulatedPartKind,
    BuildingCollapseVisual, CachedArticulatedPart, DamageVisualTier, FogOfWarPlaneEntity,
    PlacementGhostModel, ShieldFresnelKind, SimVisualEntity, UnitPbrTeamMaterial,
    VehicleWreckVisual,
};
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use rts_sdk::schema::VisualDef;

pub type ArticulatedNodeFilter = (
    Without<SimVisualEntity>,
    Without<BuildingCollapseVisual>,
    Without<VehicleWreckVisual>,
    Without<FogOfWarPlaneEntity>,
    Without<PlacementGhostModel>,
    Without<preview_stage::PreviewStageEntity>,
);

/// Discovered `Wheel_*` glTF child node on a ground vehicle (`Wheel_FL`, `Wheel_01`, `Wheel_*_LOD1`, etc.).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscoveredWheelNode {
    pub entity: Entity,
    pub rest_translation: Vec3,
    pub rest_rotation: Quat,
    pub radius: f32,
}

pub struct ScenePbrUpgradeCtx<'a> {
    pub std_mats: Option<&'a Assets<StandardMaterial>>,
    pub ext_mats: Option<&'a mut Assets<UnitPbrTeamMaterial>>,
    pub fow_tex: Handle<Image>,
    pub micro_tex: Handle<Image>,
    pub world_size: Vec2,
    pub fow_strength: f32,
    pub strategic_fade: f32,
    pub elapsed: f32,
}

impl ScenePbrUpgradeCtx<'_> {
    pub fn refresh_handles(&mut self, handles: &[Handle<UnitPbrTeamMaterial>]) {
        if let Some(ref mut ext) = self.ext_mats {
            for h in handles {
                if let Some(mat) = ext.get_mut(h) {
                    mat.extension.params.damage_and_fx.w = self.strategic_fade;
                    mat.extension.params.map_fow_params = Vec4::new(
                        self.world_size.x,
                        self.world_size.y,
                        self.fow_strength,
                        self.elapsed,
                    );
                }
            }
        }
    }
}

#[inline]
pub fn set_node_translation_if_dirty(tf: &mut Mut<Transform>, target: Vec3) {
    if tf.translation.distance_squared(target) > 1e-8 {
        tf.translation = target;
    }
}

#[inline]
pub fn set_node_rotation_if_dirty(tf: &mut Mut<Transform>, target: Quat) {
    if !tf.rotation.abs_diff_eq(target, 1e-4) && !tf.rotation.abs_diff_eq(-target, 1e-4) {
        tf.rotation = target;
    }
}

#[inline]
pub fn resolve_entity_base_mult_and_iso_yaw(
    loaded: &rts_sdk::loader::LoadedMod,
    type_id: &str,
    building_footprint: Option<[u32; 2]>,
    is_infantry: bool,
) -> (f32, f32) {
    let vdef = if building_footprint.is_some() {
        loaded.buildings.get(type_id).map(|b| &b.visual)
    } else {
        loaded.units.get(type_id).map(|u| &u.visual)
    };
    if vdef.is_some_and(|v| v.team_mask_mode == rts_sdk::schema::TeamMaskMode::AlphaMask) {
        return (vdef.map(|v| v.scale[0].max(0.05)).unwrap_or(1.0), 0.0);
    }
    let mult = if let Some([fw, fh]) = building_footprint {
        (fw.max(fh) as f32) * 0.96
    } else if is_infantry {
        0.95
    } else {
        0.78
    };
    let yaw = if building_footprint.is_some() {
        38.0_f32.to_radians()
    } else {
        0.0
    };
    (mult, yaw)
}

#[inline]
pub fn is_wheel_node_name(name: &str) -> bool {
    name.starts_with("Wheel")
}

#[inline]
pub fn compute_wheel_spin_delta(prev_pos: Vec3, curr_pos: Vec3, hull_yaw: f32, radius: f32) -> f32 {
    let delta = curr_pos - prev_pos;
    if delta.length_squared() > 25.0 {
        return 0.0;
    }
    let signed_forward = delta.x * hull_yaw.sin() + delta.z * hull_yaw.cos();
    signed_forward / radius.max(0.12)
}

pub fn discover_articulated_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) -> Option<Vec<CachedArticulatedPart>> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let (mut any_named, mut parts, mut stack) = (false, Vec::new(), vec![(root, false)]);
    while let Some((curr, anc_turret)) = stack.pop() {
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                let mut child_anc_turret = anc_turret;
                if let Ok((name, tf)) = name_q.get(child) {
                    any_named = true;
                    let kind_opt = match name.as_str() {
                        "Turret" | "Turret_LOD1" => {
                            child_anc_turret |= name.as_str() == "Turret";
                            Some(ArticulatedPartKind::Turret)
                        }
                        "Barrel" | "Barrel_LOD1" => Some(ArticulatedPartKind::Barrel),
                        "Rotor" | "Rotor_LOD1" => Some(ArticulatedPartKind::Rotor),
                        _ => None,
                    };
                    if let Some(kind) = kind_opt {
                        parts.push(CachedArticulatedPart {
                            entity: child,
                            kind,
                            rest_translation: tf.translation,
                            parent_is_turret: anc_turret,
                        });
                    }
                }
                stack.push((child, child_anc_turret));
            }
        }
    }
    any_named.then_some(parts)
}

pub fn discover_wheel_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) -> Option<Vec<DiscoveredWheelNode>> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let (mut any_named, mut wheels, mut stack) = (false, Vec::new(), vec![root]);
    while let Some(curr) = stack.pop() {
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                if let Ok((name, tf)) = name_q.get(child) {
                    any_named = true;
                    if is_wheel_node_name(name.as_str()) {
                        let est_r = if tf.translation.y > 0.08 {
                            tf.translation.y.clamp(0.18, 0.72)
                        } else {
                            0.34
                        };
                        wheels.push(DiscoveredWheelNode {
                            entity: child,
                            rest_translation: tf.translation,
                            rest_rotation: tf.rotation,
                            radius: est_r,
                        });
                    }
                }
                stack.push(child);
            }
        }
    }
    any_named.then_some(wheels)
}

pub fn apply_cached_articulation(
    parts: &[CachedArticulatedPart],
    rel_yaw: f32,
    turret_elevation_rad: f32,
    barrel_recoil_z: f32,
    rotor_spin_angle: f32,
    turret_node_q: &mut Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) {
    let has_barrel = parts
        .iter()
        .any(|p| matches!(p.kind, ArticulatedPartKind::Barrel));
    for part in parts {
        if let Ok((_, mut tf)) = turret_node_q.get_mut(part.entity) {
            let (desired_rot, desired_pos) = compute_articulated_part_pose(
                part,
                has_barrel,
                rel_yaw,
                turret_elevation_rad,
                barrel_recoil_z,
                rotor_spin_angle,
            );
            set_node_rotation_if_dirty(&mut tf, desired_rot);
            set_node_translation_if_dirty(&mut tf, desired_pos);
        }
    }
}

pub fn apply_wheel_spin(
    wheels: &[DiscoveredWheelNode],
    wheel_spin_rad: f32,
    node_q: &mut Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) {
    if wheels.is_empty() {
        return;
    }
    let spin_quat = Quat::from_rotation_x(wheel_spin_rad);
    for wheel in wheels {
        if let Ok((_, mut tf)) = node_q.get_mut(wheel.entity) {
            set_node_rotation_if_dirty(&mut tf, (wheel.rest_rotation * spin_quat).normalize());
            set_node_translation_if_dirty(&mut tf, wheel.rest_translation);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn upgrade_scene_hierarchy_to_unit_pbr<F: QueryFilter>(
    commands: &mut Commands,
    root: Entity,
    skip_unnamed_root_child: bool,
    children_q: &Query<&Children>,
    node_q: &Query<
        (
            Option<&Name>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        F,
    >,
    ctx: &mut ScenePbrUpgradeCtx<'_>,
    vdef: &VisualDef,
    tint_rgba: [f32; 4],
    tier: DamageVisualTier,
    shield: ShieldFresnelKind,
) -> Option<Vec<Handle<UnitPbrTeamMaterial>>> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let (mut found, mut orig_mats, mut stack) = (Vec::new(), Vec::new(), vec![(root, false)]);
    while let Some((curr, anc_lod1)) = stack.pop() {
        if let Ok(ch) = children_q.get(curr) {
            for &c in ch.iter() {
                let mut is_lod1 = anc_lod1;
                if let Ok((name_opt, m_opt, mat_opt)) = node_q.get(c) {
                    is_lod1 |= name_opt.is_some_and(|n| is_lod1_node_name(n.as_str()));
                    let skip = skip_unnamed_root_child && curr == root && name_opt.is_none();
                    if m_opt.is_some() && !skip {
                        found.push((c, is_lod1));
                        orig_mats.push(mat_opt.map(|m| m.0.clone()));
                    }
                }
                stack.push((c, is_lod1));
            }
        }
    }
    if found.is_empty() {
        return None;
    }
    for &(m_ent, is_lod1) in &found {
        commands.entity(m_ent).insert(if is_lod1 {
            lod1_visibility_range()
        } else {
            lod0_visibility_range()
        });
    }
    let mut handles = Vec::new();
    if let (Some(sm), Some(ref mut ext)) = (ctx.std_mats, ctx.ext_mats.as_deref_mut()) {
        let fallback = StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.58,
            metallic: 0.30,
            ..default()
        };
        for (idx, &(m_ent, _)) in found.iter().enumerate() {
            let base_ref = orig_mats
                .get(idx)
                .and_then(|o| o.as_ref())
                .and_then(|h| sm.get(h))
                .unwrap_or(&fallback);
            let mut m = build_unit_pbr_team_material(
                base_ref,
                Some(vdef),
                tint_rgba,
                true,
                tier,
                shield,
                None,
                None,
                ctx.fow_tex.clone(),
                ctx.world_size,
                ctx.fow_strength,
                ctx.strategic_fade,
                ctx.elapsed,
            );
            m.extension.micro_detail_tex = ctx.micro_tex.clone();
            let h = ext.add(m);
            handles.push(h.clone());
            commands
                .entity(m_ent)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(h));
        }
    }
    Some(handles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_spin_delta_matches_signed_forward_travel_and_ignores_teleports() {
        let fwd = compute_wheel_spin_delta(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.68), 0.0, 0.34);
        assert!((fwd - 2.0).abs() < 1e-4);
        let rev = compute_wheel_spin_delta(Vec3::new(0.0, 0.0, 0.68), Vec3::ZERO, 0.0, 0.34);
        assert!((rev + 2.0).abs() < 1e-4);
        assert_eq!(
            compute_wheel_spin_delta(Vec3::ZERO, Vec3::new(0.0, 0.0, 40.0), 0.0, 0.34),
            0.0
        );
        assert!(is_wheel_node_name("Wheel_FL") && is_wheel_node_name("Wheel_01_LOD1"));
        assert!(!is_wheel_node_name("Turret"));
    }
}
