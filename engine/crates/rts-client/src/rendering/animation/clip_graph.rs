use super::MAX_PROCEDURAL_ANIM_DISTANCE;
use bevy::prelude::*;
use rts_sdk::schema::common::{AnimClipMap, EntityAnimState};
use rts_simulation::world::SimEntity;
use std::collections::{BTreeMap, HashMap};

/// Returns `true` when an entity's procedural/glTF rig animation should be skipped because it is
/// shrouded in Fog-of-War (`Visibility::Hidden`), beyond `MAX_PROCEDURAL_ANIM_DISTANCE`, or outside
/// the active camera `Frustum` (`Task 205b`).
pub fn should_cull_entity_animation(
    visibility: Visibility,
    entity_pos: Vec3,
    camera_pos: Option<Vec3>,
    camera_frustum: Option<&bevy::render::primitives::Frustum>,
    bound_radius: f32,
) -> bool {
    if visibility == Visibility::Hidden {
        return true;
    }
    if let Some(cam_p) = camera_pos {
        let max_d2 = MAX_PROCEDURAL_ANIM_DISTANCE * MAX_PROCEDURAL_ANIM_DISTANCE;
        if cam_p.distance_squared(entity_pos) > max_d2 {
            return true;
        }
    }
    if let Some(frustum) = camera_frustum {
        if frustum.half_spaces[0].normal().length_squared() > 0.5 {
            let sphere = bevy::render::primitives::Sphere {
                center: entity_pos.into(),
                radius: bound_radius.max(1.5),
            };
            if !frustum.intersects_sphere(&sphere, true) {
                return true;
            }
        }
    }
    false
}

/// Pure state-transition selector for unit/infantry animations (`Idle`, `Walk`, `Prone`, `Attack`, `Deploy`, `Die`).
pub fn select_unit_anim_state(
    sim_ent: Option<&SimEntity>,
    is_dying: bool,
    attack_hold_secs: f32,
) -> EntityAnimState {
    if is_dying {
        return EntityAnimState::Die;
    }
    let Some(ent) = sim_ent else {
        return EntityAnimState::Die;
    };
    if ent.health <= 0.0 {
        return EntityAnimState::Die;
    }
    if ent.recoil_timer > 0.02 || attack_hold_secs > 0.0 {
        return EntityAnimState::Attack;
    }
    if ent.suppression >= 0.5 {
        return EntityAnimState::Prone;
    }
    if ent.current_speed > 0.12 || !ent.path_waypoints.is_empty() {
        return EntityAnimState::Walk;
    }
    if ent.script_ctx.deployed {
        return EntityAnimState::Deploy;
    }
    EntityAnimState::Idle
}

/// Resolves the best-matching glTF clip name for `state`, honoring explicit `VisualDef.animations`
/// overrides first before falling back to canonical case-insensitive clip aliases.
pub fn resolve_clip_name_for_state<'a>(
    state: EntityAnimState,
    custom_map: &AnimClipMap,
    available_names: &[&'a str],
) -> Option<&'a str> {
    if available_names.is_empty() {
        return None;
    }

    if let Some(explicit) = custom_map.clip_for_state(state) {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            if let Some(&exact) = available_names.iter().find(|&&n| n == trimmed) {
                return Some(exact);
            }
            if let Some(&ci) = available_names
                .iter()
                .find(|&&n| n.eq_ignore_ascii_case(trimmed))
            {
                return Some(ci);
            }
        }
    }

    let aliases: &[&str] = match state {
        EntityAnimState::Idle => &["idle", "stand", "ready", "breathe", "default"],
        EntityAnimState::Walk => &["walk", "move", "run", "march", "locomotion"],
        EntityAnimState::Prone => &["prone", "crawl", "suppressed", "crouch"],
        EntityAnimState::Attack => &["attack", "fire", "shoot", "combat", "strike"],
        EntityAnimState::Deploy => &["deploy", "unpack", "fortify", "setup"],
        EntityAnimState::Die => &["die", "death", "dead", "collapse", "fall"],
        EntityAnimState::Constructing => &["constructing", "construct", "build", "buildup"],
        EntityAnimState::Producing => &["producing", "produce", "rollout", "factory"],
        EntityAnimState::Damaged => &["damaged", "critical", "hurt", "broken"],
    };

    for &alias in aliases {
        if let Some(&matched) = available_names
            .iter()
            .find(|&&n| n.eq_ignore_ascii_case(alias))
        {
            return Some(matched);
        }
    }
    for &alias in aliases {
        if let Some(&matched) = available_names
            .iter()
            .find(|&&n| n.to_ascii_lowercase().contains(alias))
        {
            return Some(matched);
        }
    }
    None
}

/// Builds a Bevy `AnimationGraph` and `EntityAnimState -> AnimationNodeIndex` table from embedded glTF clips.
pub fn build_animation_graph_from_clips(
    named_clips: &[(String, Handle<AnimationClip>)],
    indexed_clips: &[Handle<AnimationClip>],
    custom_map: &AnimClipMap,
) -> Option<(
    AnimationGraph,
    BTreeMap<EntityAnimState, AnimationNodeIndex>,
)> {
    if named_clips.is_empty() && indexed_clips.is_empty() {
        return None;
    }

    let mut graph = AnimationGraph::new();
    let mut handle_to_node: HashMap<AssetId<AnimationClip>, AnimationNodeIndex> = HashMap::new();
    let mut name_to_node: HashMap<String, AnimationNodeIndex> = HashMap::new();

    let mut sorted_named: Vec<&(String, Handle<AnimationClip>)> = named_clips.iter().collect();
    sorted_named.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, clip_h) in &sorted_named {
        let idx = *handle_to_node
            .entry(clip_h.id())
            .or_insert_with(|| graph.add_clip((*clip_h).clone(), 1.0, graph.root));
        name_to_node.insert(name.clone(), idx);
    }

    let mut first_indexed_node: Option<AnimationNodeIndex> = None;
    for clip_h in indexed_clips {
        let idx = *handle_to_node
            .entry(clip_h.id())
            .or_insert_with(|| graph.add_clip(clip_h.clone(), 1.0, graph.root));
        if first_indexed_node.is_none() {
            first_indexed_node = Some(idx);
        }
    }

    let avail_strs: Vec<&str> = sorted_named.iter().map(|(n, _)| n.as_str()).collect();
    let all_states = [
        EntityAnimState::Idle,
        EntityAnimState::Walk,
        EntityAnimState::Prone,
        EntityAnimState::Attack,
        EntityAnimState::Deploy,
        EntityAnimState::Die,
        EntityAnimState::Constructing,
        EntityAnimState::Producing,
        EntityAnimState::Damaged,
    ];

    let mut state_nodes = BTreeMap::new();
    for st in all_states {
        if let Some(matched_name) = resolve_clip_name_for_state(st, custom_map, &avail_strs) {
            if let Some(&node_idx) = name_to_node.get(matched_name) {
                state_nodes.insert(st, node_idx);
            }
        }
    }

    if let std::collections::btree_map::Entry::Vacant(vacant) =
        state_nodes.entry(EntityAnimState::Idle)
    {
        if let Some(fallback_idle) = first_indexed_node.or_else(|| {
            sorted_named
                .first()
                .and_then(|(n, _)| name_to_node.get(n).copied())
        }) {
            vacant.insert(fallback_idle);
        }
    }

    if state_nodes.is_empty() {
        None
    } else {
        Some((graph, state_nodes))
    }
}

#[inline]
pub fn anim_state_is_looping(state: EntityAnimState) -> bool {
    matches!(
        state,
        EntityAnimState::Idle
            | EntityAnimState::Walk
            | EntityAnimState::Prone
            | EntityAnimState::Deploy
            | EntityAnimState::Producing
            | EntityAnimState::Damaged
    )
}

#[allow(clippy::type_complexity)]
pub(super) fn evaluate_procedural_limb_pose(
    state: EntityAnimState,
    t: f32,
    sim_ent: Option<&SimEntity>,
    death_anim_progress: Option<f32>,
    state_elapsed: f32,
) -> (Vec3, Quat, Quat, Quat, Quat, Quat) {
    match state {
        EntityAnimState::Idle => {
            let breath = (t * 2.4).sin() * 0.025;
            let scan = (t * 1.1).sin() * 0.16;
            (
                Vec3::new(0.0, breath, 0.0),
                Quat::from_rotation_y(scan * 0.25),
                Quat::from_rotation_y(scan),
                Quat::IDENTITY,
                Quat::IDENTITY,
                Quat::from_rotation_x(-0.22 + breath * 0.8),
            )
        }
        EntityAnimState::Walk => {
            let stride = (t * 9.5).sin();
            let bounce = (t * 19.0).cos().abs() * 0.055;
            (
                Vec3::new(0.0, bounce, 0.0),
                Quat::from_euler(EulerRot::YXZ, -stride * 0.12, 0.10, 0.0),
                Quat::from_rotation_y(stride * 0.08),
                Quat::from_rotation_x(stride * 0.65),
                Quat::from_rotation_x(-stride * 0.65),
                Quat::from_rotation_x(-0.42 + stride * 0.22),
            )
        }
        EntityAnimState::Prone => {
            let moving = sim_ent.is_some_and(|e| !e.path_waypoints.is_empty());
            let crawl = if moving { (t * 5.5).sin() * 0.28 } else { 0.0 };
            (
                Vec3::new(0.0, -0.34, 0.08),
                Quat::from_rotation_x(1.28),
                Quat::from_rotation_x(-0.92),
                Quat::from_euler(EulerRot::XYZ, -1.15 + crawl, 0.0, -0.18),
                Quat::from_euler(EulerRot::XYZ, -1.15 - crawl, 0.0, 0.18),
                Quat::from_rotation_x(-1.10 + crawl * 0.5),
            )
        }
        EntityAnimState::Attack => {
            let recoil = sim_ent
                .map(|e| e.recoil_timer.clamp(0.0, 0.35))
                .unwrap_or(0.0);
            let kick = recoil * 1.35;
            (
                Vec3::new(0.0, -0.03, -recoil * 0.12),
                Quat::from_euler(EulerRot::YXZ, -0.18, -kick * 0.22, 0.0),
                Quat::from_rotation_y(0.16),
                Quat::from_rotation_x(-0.18),
                Quat::from_rotation_x(0.22),
                Quat::from_rotation_x(-1.18 - kick * 0.65),
            )
        }
        EntityAnimState::Deploy => {
            let breath = (t * 1.8).sin() * 0.015;
            (
                Vec3::new(0.0, -0.18 + breath, 0.04),
                Quat::from_rotation_x(0.24),
                Quat::from_rotation_x(-0.18),
                Quat::from_rotation_x(-0.62),
                Quat::from_rotation_x(0.58),
                Quat::from_rotation_x(-1.05),
            )
        }
        EntityAnimState::Die => {
            let p = death_anim_progress
                .unwrap_or((state_elapsed / 0.55).clamp(0.0, 1.0))
                .clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - p).powi(2);
            (
                Vec3::new(0.0, -0.46 * eased, -0.18 * eased),
                Quat::from_euler(EulerRot::XYZ, -1.46 * eased, 0.22 * eased, 0.15 * eased),
                Quat::from_rotation_x(-0.35 * eased),
                Quat::from_rotation_x(0.45 * eased),
                Quat::from_rotation_x(0.25 * eased),
                Quat::from_euler(EulerRot::XYZ, 0.65 * eased, 0.0, 0.55 * eased),
            )
        }
        _ => (
            Vec3::ZERO,
            Quat::IDENTITY,
            Quat::IDENTITY,
            Quat::IDENTITY,
            Quat::IDENTITY,
            Quat::IDENTITY,
        ),
    }
}
