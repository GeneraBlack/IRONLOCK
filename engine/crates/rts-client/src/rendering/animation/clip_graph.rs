use super::MAX_PROCEDURAL_ANIM_DISTANCE;
use bevy::prelude::*;
use rts_sdk::schema::common::{AnimClipMap, EntityAnimState};
use rts_simulation::world::SimEntity;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

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

/// Resolution result when inspecting embedded glTF clips for an articulated `"Constructing"` clip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstructClipResolution {
    /// Matched `"Constructing"` clip handle exists, but `Assets<AnimationClip>` has not committed it yet.
    PendingAsset,
    /// Clip duration inspection succeeded (`articulated_buildup == true` iff `duration > 0.05`).
    Ready {
        constructing_duration: Option<f32>,
        articulated_buildup: bool,
    },
}

/// Inspects `named_clips` for a `"Constructing"` clip and resolves its exact duration (`> 0.05s`).
pub fn resolve_constructing_clip_duration(
    named_clips: &[(String, Handle<AnimationClip>)],
    custom_map: &AnimClipMap,
    clip_assets: Option<&Assets<AnimationClip>>,
) -> ConstructClipResolution {
    if named_clips.is_empty() {
        return ConstructClipResolution::Ready {
            constructing_duration: None,
            articulated_buildup: false,
        };
    }
    let mut sorted_names: Vec<&str> = named_clips.iter().map(|(n, _)| n.as_str()).collect();
    sorted_names.sort_unstable();
    let Some(matched_name) =
        resolve_clip_name_for_state(EntityAnimState::Constructing, custom_map, &sorted_names)
    else {
        return ConstructClipResolution::Ready {
            constructing_duration: None,
            articulated_buildup: false,
        };
    };
    let Some((_, clip_handle)) = named_clips.iter().find(|(n, _)| n == matched_name) else {
        return ConstructClipResolution::Ready {
            constructing_duration: None,
            articulated_buildup: false,
        };
    };
    let Some(clips) = clip_assets else {
        return ConstructClipResolution::Ready {
            constructing_duration: None,
            articulated_buildup: false,
        };
    };
    let Some(clip) = clips.get(clip_handle) else {
        return ConstructClipResolution::PendingAsset;
    };
    let dur = clip.duration();
    if dur > 0.05 {
        ConstructClipResolution::Ready {
            constructing_duration: Some(dur),
            articulated_buildup: true,
        }
    } else {
        ConstructClipResolution::Ready {
            constructing_duration: None,
            articulated_buildup: false,
        }
    }
}

/// Computes normalized `0.0..=1.0` bidirectional `"Constructing"` scrub progress for buildings:
/// - `deconstruct_progress` (`1.0 -> 0.0` reverse scrub on sell or ConYard undeploy)
/// - `spawn_progress < 0.996` (`0.0 -> 1.0` forward scrub on build or MCV deploy)
/// - `None` once online (`spawn_progress >= 0.996 && deconstruct_progress.is_none()`).
#[inline]
pub fn compute_building_scrub_progress(
    is_building: bool,
    spawn_progress: f32,
    deconstruct_progress: Option<f32>,
) -> Option<f32> {
    if let Some(dec_p) = deconstruct_progress {
        Some(dec_p.clamp(0.0, 1.0))
    } else if is_building && spawn_progress < 0.996 {
        Some(spawn_progress.clamp(0.0, 1.0))
    } else {
        None
    }
}

/// Maps normalized `0.0..=1.0` buildup/builddown progress onto `[0.0, (dur - 1e-4).max(0.0)]`.
#[inline]
pub fn compute_construct_scrub_time(progress_01: f32, duration: f32) -> f32 {
    let p = progress_01.clamp(0.0, 1.0);
    let max_t = (duration - 1e-4).max(0.0);
    (p * duration).clamp(0.0, max_t)
}

/// Drives `construct_node` in paused exact-frame scrub mode (`Duration::ZERO` instant switch).
pub fn apply_construct_scrub_to_player(
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    construct_node: AnimationNodeIndex,
    seek_t: f32,
    force_switch: bool,
) {
    if force_switch || !player.is_playing_animation(construct_node) {
        player.stop_all();
        *transitions = AnimationTransitions::new();
        let active = transitions.play(player, construct_node, Duration::ZERO);
        active.pause();
        active.seek_to(seek_t);
    } else if let Some(active) = player.animation_mut(construct_node) {
        active.pause();
        active.seek_to(seek_t);
    }
}

/// Snaps `construct_node` to its exact `100%` rest pose `(dur - 1e-4).max(0.0)` and resumes it
/// right before crossfading (`120ms`) into `Idle` / `Producing` so Bevy's `AnimationTransitions`
/// cleanly fades out and expires `construct_node` without leaving any sub-assembly offset.
pub fn snap_construct_to_rest_before_transition(
    player: &mut AnimationPlayer,
    construct_node: AnimationNodeIndex,
    duration: f32,
    resume_for_crossfade: bool,
) {
    let snap_t = compute_construct_scrub_time(1.0, duration);
    if let Some(active) = player.animation_mut(construct_node) {
        active.seek_to(snap_t);
        if resume_for_crossfade {
            active.resume();
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn articulated_buildup_detects_constructing_clip_duration_and_pending_assets() {
        let mut clips = Assets::<AnimationClip>::default();
        let mut valid_construct = AnimationClip::default();
        valid_construct.set_duration(1.20);
        let h_valid = clips.add(valid_construct);

        let mut tiny_construct = AnimationClip::default();
        tiny_construct.set_duration(0.02);
        let h_tiny = clips.add(tiny_construct);
        let h_idle = clips.add(AnimationClip::default());

        let named_valid = vec![
            ("Idle".to_string(), h_idle.clone()),
            ("Constructing".to_string(), h_valid),
        ];
        let res_valid =
            resolve_constructing_clip_duration(&named_valid, &AnimClipMap::default(), Some(&clips));
        assert_eq!(
            res_valid,
            ConstructClipResolution::Ready {
                constructing_duration: Some(1.20),
                articulated_buildup: true,
            }
        );

        let named_tiny = vec![
            ("Idle".to_string(), h_idle.clone()),
            ("Constructing".to_string(), h_tiny),
        ];
        assert_eq!(
            resolve_constructing_clip_duration(&named_tiny, &AnimClipMap::default(), Some(&clips)),
            ConstructClipResolution::Ready {
                constructing_duration: None,
                articulated_buildup: false,
            }
        );

        let named_no_construct = vec![("Idle".to_string(), h_idle)];
        assert_eq!(
            resolve_constructing_clip_duration(
                &named_no_construct,
                &AnimClipMap::default(),
                Some(&clips)
            ),
            ConstructClipResolution::Ready {
                constructing_duration: None,
                articulated_buildup: false,
            }
        );

        let pending_handle = Handle::<AnimationClip>::weak_from_u128(0xdead_beef_cafe_1234);
        let named_pending = vec![("Constructing".to_string(), pending_handle)];
        assert_eq!(
            resolve_constructing_clip_duration(
                &named_pending,
                &AnimClipMap::default(),
                Some(&clips)
            ),
            ConstructClipResolution::PendingAsset
        );
    }

    #[test]
    fn bidirectional_construct_scrub_maps_forward_buildup_reverse_builddown_and_rest_snap() {
        let dur = 1.20_f32;
        assert_eq!(
            compute_building_scrub_progress(true, 0.04, None),
            Some(0.04)
        );
        assert_eq!(
            compute_building_scrub_progress(true, 0.78, None),
            Some(0.78)
        );
        assert_eq!(compute_building_scrub_progress(true, 0.996, None), None);
        assert_eq!(compute_building_scrub_progress(true, 1.0, None), None);
        assert_eq!(compute_building_scrub_progress(false, 0.25, None), None);
        assert_eq!(
            compute_building_scrub_progress(true, 1.0, Some(0.42)),
            Some(0.42)
        );

        assert!((compute_construct_scrub_time(0.0, dur) - 0.0).abs() < 1e-6);
        assert!((compute_construct_scrub_time(0.5, dur) - 0.60).abs() < 1e-6);
        assert!((compute_construct_scrub_time(1.0, dur) - (dur - 1e-4)).abs() < 1e-6);

        let mut clips = Assets::<AnimationClip>::default();
        let mut c_clip = AnimationClip::default();
        c_clip.set_duration(dur);
        let h_con = clips.add(c_clip);
        let h_idle = clips.add(AnimationClip::default());
        let named = vec![
            ("Constructing".to_string(), h_con),
            ("Idle".to_string(), h_idle),
        ];
        let (_graph, nodes) =
            build_animation_graph_from_clips(&named, &[], &AnimClipMap::default()).unwrap();
        let con_node = nodes[&EntityAnimState::Constructing];
        let idle_node = nodes[&EntityAnimState::Idle];

        let mut player = AnimationPlayer::default();
        let mut transitions = AnimationTransitions::new();

        // Forward scrub 0.18 -> 0.76
        apply_construct_scrub_to_player(
            &mut player,
            &mut transitions,
            con_node,
            compute_construct_scrub_time(0.18, dur),
            true,
        );
        let a18 = player.animation(con_node).expect("construct active");
        assert!(a18.is_paused());
        assert!((a18.seek_time() - 0.18 * dur).abs() < 1e-5);

        apply_construct_scrub_to_player(
            &mut player,
            &mut transitions,
            con_node,
            compute_construct_scrub_time(0.76, dur),
            false,
        );
        assert!((player.animation(con_node).unwrap().seek_time() - 0.76 * dur).abs() < 1e-5);

        // Completion snap (`was_scrubbing_construct: true -> false`) + crossfade to Idle
        snap_construct_to_rest_before_transition(&mut player, con_node, dur, true);
        let snapped = player.animation(con_node).unwrap();
        assert!(!snapped.is_paused());
        assert!((snapped.seek_time() - (dur - 1e-4)).abs() < 1e-5);
        transitions
            .play(&mut player, idle_node, Duration::from_millis(120))
            .repeat();
        assert_eq!(transitions.get_main_animation(), Some(idle_node));

        // Reverse scrub on sell / ConYard undeploy (`1.0 -> 0.42 -> 0.05`)
        for &rev_p in &[1.0_f32, 0.42, 0.05] {
            apply_construct_scrub_to_player(
                &mut player,
                &mut transitions,
                con_node,
                compute_construct_scrub_time(rev_p, dur),
                rev_p >= 0.99,
            );
            let a_rev = player.animation(con_node).unwrap();
            assert!(a_rev.is_paused());
            let expected = compute_construct_scrub_time(rev_p, dur);
            assert!((a_rev.seek_time() - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn anim_controller_transitions_across_idle_walk_prone_attack_and_die() {
        let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2 path");
        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(mod_dir).expect("load rust-ra2");
        let mut world = rts_simulation::world::SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        let id = world.entities.keys().copied().next().expect("entity");
        let ent = world.entities.get_mut(&id).unwrap();
        ent.health = ent.max_health;
        ent.recoil_timer = 0.0;
        ent.suppression = 0.0;
        ent.current_speed = 0.0;
        ent.path_waypoints.clear();

        assert_eq!(
            select_unit_anim_state(Some(ent), false, 0.0),
            EntityAnimState::Idle
        );
        ent.script_ctx.deployed = true;
        assert_eq!(
            select_unit_anim_state(Some(ent), false, 0.0),
            EntityAnimState::Deploy
        );
        ent.script_ctx.deployed = false;
        ent.current_speed = 3.2;
        assert_eq!(
            select_unit_anim_state(Some(ent), false, 0.0),
            EntityAnimState::Walk
        );
        ent.suppression = 0.65;
        assert_eq!(
            select_unit_anim_state(Some(ent), false, 0.0),
            EntityAnimState::Prone
        );
        ent.recoil_timer = 0.15;
        assert_eq!(
            select_unit_anim_state(Some(ent), false, 0.0),
            EntityAnimState::Attack
        );
        assert_eq!(
            select_unit_anim_state(Some(ent), true, 0.0),
            EntityAnimState::Die
        );
    }

    #[test]
    fn gltf_clip_resolution_and_animation_graph_builder_honor_custom_and_alias_names() {
        let mut clips_assets = Assets::<AnimationClip>::default();
        let idle_h = clips_assets.add(AnimationClip::default());
        let move_h = clips_assets.add(AnimationClip::default());
        let fire_h = clips_assets.add(AnimationClip::default());
        let custom_die_h = clips_assets.add(AnimationClip::default());

        let named = vec![
            ("Armature|Stand_Ready".to_string(), idle_h),
            ("Run_Forward".to_string(), move_h),
            ("Rifle_Fire".to_string(), fire_h),
            ("CustomRagdollA".to_string(), custom_die_h),
        ];
        let custom_map = AnimClipMap {
            die: Some("CustomRagdollA".to_string()),
            ..Default::default()
        };

        let (_graph, state_nodes) = build_animation_graph_from_clips(&named, &[], &custom_map)
            .expect("non-empty clips must produce AnimationGraph");
        assert!(state_nodes.contains_key(&EntityAnimState::Idle));
        assert!(state_nodes.contains_key(&EntityAnimState::Walk));
        assert!(state_nodes.contains_key(&EntityAnimState::Attack));
        assert!(state_nodes.contains_key(&EntityAnimState::Die));
        assert_ne!(
            state_nodes[&EntityAnimState::Walk],
            state_nodes[&EntityAnimState::Attack]
        );
        assert!(build_animation_graph_from_clips(&[], &[], &AnimClipMap::default()).is_none());
    }

    #[test]
    fn entity_animation_culls_hidden_and_distant_units() {
        let cam = Some(Vec3::new(50.0, 35.0, 50.0));
        assert!(should_cull_entity_animation(
            Visibility::Hidden,
            Vec3::new(50.0, 0.0, 50.0),
            cam,
            None,
            2.0
        ));
        assert!(!should_cull_entity_animation(
            Visibility::Inherited,
            Vec3::new(60.0, 0.0, 60.0),
            cam,
            None,
            2.0
        ));
        assert!(should_cull_entity_animation(
            Visibility::Inherited,
            Vec3::new(220.0, 0.0, 220.0),
            cam,
            None,
            2.0
        ));
    }
}
