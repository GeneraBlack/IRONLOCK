use super::{set_rotation_if_dirty, set_translation_if_dirty};
use crate::rendering::{
    compute_articulated_part_pose, preview_stage, ArticulatedPartKind, CachedArticulatedPart,
    FogOfWarPlaneEntity, PlacementGhostModel, SimVisualEntity,
};
use bevy::prelude::*;

pub(super) type ArticulatedNodeFilter = (
    Without<SimVisualEntity>,
    Without<FogOfWarPlaneEntity>,
    Without<PlacementGhostModel>,
    Without<preview_stage::PreviewStageEntity>,
);

#[inline]
pub(super) fn resolve_entity_base_mult_and_iso_yaw(
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

pub(super) fn discover_articulated_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) -> Option<Vec<CachedArticulatedPart>> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let mut any_named = false;
    let mut parts = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((curr, anc_turret)) = stack.pop() {
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                let mut child_anc_turret = anc_turret;
                if let Ok((name, tf)) = name_q.get(child) {
                    any_named = true;
                    let kind_opt = match name.as_str() {
                        "Turret" => {
                            child_anc_turret = true;
                            Some(ArticulatedPartKind::Turret)
                        }
                        "Barrel" => Some(ArticulatedPartKind::Barrel),
                        "Rotor" => Some(ArticulatedPartKind::Rotor),
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

pub(super) fn apply_cached_articulation(
    parts: &[CachedArticulatedPart],
    rel_yaw: f32,
    turret_elevation_rad: f32,
    barrel_recoil_z: f32,
    rotor_spin_angle: f32,
    turret_node_q: &mut Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) {
    let has_separate_barrel = parts
        .iter()
        .any(|p| matches!(p.kind, ArticulatedPartKind::Barrel));
    for part in parts {
        if let Ok((_, mut tf)) = turret_node_q.get_mut(part.entity) {
            let (desired_rot, desired_pos) = compute_articulated_part_pose(
                part,
                has_separate_barrel,
                rel_yaw,
                turret_elevation_rad,
                barrel_recoil_z,
                rotor_spin_angle,
            );
            set_rotation_if_dirty(&mut tf, desired_rot);
            set_translation_if_dirty(&mut tf, desired_pos);
        }
    }
}
