mod clip_graph;

pub use clip_graph::{
    anim_state_is_looping, apply_construct_scrub_to_player, build_animation_graph_from_clips,
    compute_building_scrub_progress, compute_construct_scrub_time, resolve_clip_name_for_state,
    resolve_constructing_clip_duration, select_unit_anim_state, should_cull_entity_animation,
    snap_construct_to_rest_before_transition, ConstructClipResolution,
};

use super::preview_stage::PreviewStageEntity;
use super::{resolve_mod_asset_path, FogOfWarPlaneEntity, PlacementGhostModel, SimVisualEntity};
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use clip_graph::evaluate_procedural_limb_pose;
use rts_sdk::schema::common::EntityAnimState;
use rts_simulation::world::EntityKind;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

pub const GLTF_ANIM_CROSSFADE_MS: u64 = 120;
pub const MAX_PROCEDURAL_ANIM_DISTANCE: f32 = 95.0;

#[derive(Debug, Clone, Default)]
pub struct DiscoveredEntityRig {
    pub torso: Option<(Entity, Vec3)>,
    pub head: Option<(Entity, Vec3)>,
    pub left_leg: Option<(Entity, Vec3)>,
    pub right_leg: Option<(Entity, Vec3)>,
    pub weapon_arm: Option<(Entity, Vec3)>,
    pub scaffold_frame: Option<Entity>,
    pub factory_door: Option<(Entity, Vec3)>,
    pub damage_overlay: Option<Entity>,
}

#[derive(Debug, Clone)]
pub enum CachedModelAnimEntry {
    Loading(Handle<Gltf>),
    NoEmbeddedClips,
    Ready {
        graph_handle: Handle<AnimationGraph>,
        state_nodes: BTreeMap<EntityAnimState, AnimationNodeIndex>,
        constructing_duration: Option<f32>,
        articulated_buildup: bool,
    },
}

#[derive(Resource, Default, Debug)]
pub struct GltfAnimationGraphCache {
    pub entries: HashMap<(String, String), CachedModelAnimEntry>,
}

#[derive(Component, Debug, Clone)]
pub struct EntityAnimController {
    pub state: EntityAnimState,
    pub state_elapsed: f32,
    pub attack_hold_secs: f32,
    pub factory_door_open: f32,
    pub factory_rollout_timer: f32,
    pub prev_queue_len: usize,
    pub rig: Option<DiscoveredEntityRig>,
    pub anim_players: Option<Vec<Entity>>,
    pub active_gltf_node: Option<AnimationNodeIndex>,
    pub using_gltf_clips: bool,
    pub articulated_buildup: bool,
    pub was_scrubbing_construct: bool,
    pub anim_cache_resolved: bool,
}

impl Default for EntityAnimController {
    fn default() -> Self {
        Self {
            state: EntityAnimState::Idle,
            state_elapsed: 0.0,
            attack_hold_secs: 0.0,
            factory_door_open: 0.0,
            factory_rollout_timer: 0.0,
            prev_queue_len: 0,
            rig: None,
            anim_players: None,
            active_gltf_node: None,
            using_gltf_clips: false,
            articulated_buildup: false,
            was_scrubbing_construct: false,
            anim_cache_resolved: false,
        }
    }
}

#[allow(clippy::type_complexity)]
fn discover_entity_rig_and_players(
    root: Entity,
    children_q: &Query<&Children>,
    node_q: &Query<
        (&Name, &mut Transform, &mut Visibility),
        (
            Without<SimVisualEntity>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    anim_player_q: &Query<
        (
            &mut AnimationPlayer,
            Option<&mut AnimationTransitions>,
            Option<&AnimationGraphHandle>,
        ),
        Without<PreviewStageEntity>,
    >,
) -> Option<(DiscoveredEntityRig, Vec<Entity>)> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let mut any_spawned = false;
    let mut rig = DiscoveredEntityRig::default();
    let mut players = Vec::new();
    let mut stack = vec![root];
    while let Some(curr) = stack.pop() {
        if curr != root && anim_player_q.get(curr).is_ok() {
            any_spawned = true;
            players.push(curr);
        }
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                if let Ok((name, tf, _)) = node_q.get(child) {
                    any_spawned = true;
                    match name.as_str() {
                        s if s.ends_with("_LOD1") => {}
                        "Torso" => rig.torso = Some((child, tf.translation)),
                        "Head" => rig.head = Some((child, tf.translation)),
                        "LeftLeg" => rig.left_leg = Some((child, tf.translation)),
                        "RightLeg" => rig.right_leg = Some((child, tf.translation)),
                        "WeaponArm" => rig.weapon_arm = Some((child, tf.translation)),
                        "ScaffoldFrame" => rig.scaffold_frame = Some(child),
                        "FactoryDoor" => rig.factory_door = Some((child, tf.translation)),
                        "DamageOverlay" => rig.damage_overlay = Some(child),
                        _ => {}
                    }
                }
                stack.push(child);
            }
        }
    }
    if any_spawned {
        Some((rig, players))
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn step_entity_animations(
    mut commands: Commands,
    time: Res<Time>,
    (asset_server, asset_root, mod_res, sim_res): (
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
    ),
    (gltfs, anim_clips, mut anim_graphs, mut graph_cache): (
        Option<Res<Assets<Gltf>>>,
        Option<Res<Assets<AnimationClip>>>,
        Option<ResMut<Assets<AnimationGraph>>>,
        Option<ResMut<GltfAnimationGraphCache>>,
    ),
    camera_q: Query<
        (&GlobalTransform, Option<&bevy::render::primitives::Frustum>),
        (
            With<crate::camera::RtsCameraRig>,
            Without<SimVisualEntity>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut root_q: Query<
        (
            Entity,
            &SimVisualEntity,
            &Transform,
            &Visibility,
            Option<&mut EntityAnimController>,
        ),
        Without<PreviewStageEntity>,
    >,
    children_q: Query<&Children>,
    mut node_q: Query<
        (&Name, &mut Transform, &mut Visibility),
        (
            Without<SimVisualEntity>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut anim_player_q: Query<
        (
            &mut AnimationPlayer,
            Option<&mut AnimationTransitions>,
            Option<&AnimationGraphHandle>,
        ),
        Without<PreviewStageEntity>,
    >,
) {
    let Some(sim_res) = sim_res else {
        return;
    };
    let world = &sim_res.0;
    let dt = time.delta_secs();
    let active_cam = camera_q.get_single().ok();
    let cam_pos = active_cam.map(|(gtf, _)| gtf.translation());
    let cam_frustum = active_cam.and_then(|(_, f)| f);

    let mut discoveries: Vec<(Entity, DiscoveredEntityRig, Vec<Entity>)> = Vec::new();
    for (ent, _vis, _tf, root_vis, ctrl_opt) in &root_q {
        match ctrl_opt {
            None => {
                commands.entity(ent).insert(EntityAnimController::default());
            }
            Some(ctrl) if ctrl.rig.is_none() && *root_vis != Visibility::Hidden => {
                if let Some((rig, players)) =
                    discover_entity_rig_and_players(ent, &children_q, &node_q, &anim_player_q)
                {
                    discoveries.push((ent, rig, players));
                }
            }
            _ => {}
        }
    }
    for (ent, rig, players) in discoveries {
        if let Ok((_, _, _, _, Some(mut ctrl))) = root_q.get_mut(ent) {
            if players.is_empty() {
                ctrl.anim_cache_resolved = true;
                ctrl.articulated_buildup = false;
            }
            ctrl.rig = Some(rig);
            ctrl.anim_players = Some(players);
        }
    }

    for (_ent, vis, root_tf, root_vis, ctrl_opt) in &mut root_q {
        let Some(mut ctrl) = ctrl_opt else {
            continue;
        };
        let is_dying = vis.death_anim_progress.is_some();
        let active_building_scrub = vis.building_footprint.is_some()
            && (vis.spawn_progress < 0.996
                || vis.deconstruct_progress.is_some()
                || ctrl.was_scrubbing_construct);
        let bound_r = vis
            .building_footprint
            .map(|[fw, fh]| (fw.max(fh) as f32) * 1.4)
            .unwrap_or(2.2);
        if !is_dying
            && !active_building_scrub
            && should_cull_entity_animation(
                *root_vis,
                root_tf.translation,
                cam_pos,
                cam_frustum,
                bound_r,
            )
        {
            continue;
        }
        let sim_ent = world.entities.get(&vis.sim_id);

        if let Some(se) = sim_ent {
            if se.recoil_timer > 0.02 {
                ctrl.attack_hold_secs = 0.28;
            }
        }
        ctrl.attack_hold_secs = (ctrl.attack_hold_secs - dt).max(0.0);

        let next_state = select_unit_anim_state(sim_ent, is_dying, ctrl.attack_hold_secs);
        if next_state != ctrl.state {
            ctrl.state = next_state;
            ctrl.state_elapsed = 0.0;
        } else {
            ctrl.state_elapsed += dt;
        }

        if let Some(se) = sim_ent {
            if matches!(se.kind, EntityKind::Building { .. }) {
                let q_len = se.production_queue.len();
                if q_len < ctrl.prev_queue_len {
                    ctrl.factory_rollout_timer = 1.65;
                }
                ctrl.prev_queue_len = q_len;
                if se
                    .production_queue
                    .first()
                    .is_some_and(|item| item.remaining_secs <= 1.25)
                {
                    ctrl.factory_rollout_timer = ctrl.factory_rollout_timer.max(0.5);
                }
            }
        }
        if ctrl.factory_rollout_timer > 0.0 {
            ctrl.factory_rollout_timer = (ctrl.factory_rollout_timer - dt).max(0.0);
            ctrl.factory_door_open = (ctrl.factory_door_open + dt * 4.5).min(1.0);
        } else {
            ctrl.factory_door_open = (ctrl.factory_door_open - dt * 2.8).max(0.0);
        }

        let under_construction = vis.spawn_progress < 0.996 || vis.deconstruct_progress.is_some();
        let hp_ratio = sim_ent
            .map(|se| se.health / se.max_health.max(1.0))
            .unwrap_or(1.0);

        let mut drove_gltf_clips = false;
        if let Some(players) = ctrl.anim_players.clone() {
            if !players.is_empty() {
                if let (
                    Some(srv),
                    Some(aroot),
                    Some(mres),
                    Some(gltf_assets),
                    Some(graphs),
                    Some(cache),
                ) = (
                    asset_server.as_deref(),
                    asset_root.as_deref(),
                    mod_res.as_deref(),
                    gltfs.as_deref(),
                    anim_graphs.as_deref_mut(),
                    graph_cache.as_deref_mut(),
                ) {
                    let loaded = &mres.0;
                    let vdef_opt = loaded
                        .units
                        .get(&vis.entity_type_id)
                        .map(|u| &u.visual)
                        .or_else(|| loaded.buildings.get(&vis.entity_type_id).map(|b| &b.visual));
                    if let Some(vdef) = vdef_opt {
                        let scene_load =
                            resolve_mod_asset_path(&aroot.0, &loaded.root_dir, &vdef.model);
                        let gltf_load = scene_load
                            .split('#')
                            .next()
                            .unwrap_or(scene_load.as_str())
                            .to_string();
                        let cache_key = (vis.entity_type_id.clone(), gltf_load.clone());

                        if !cache.entries.contains_key(&cache_key) {
                            let h: Handle<Gltf> = srv.load(gltf_load);
                            cache
                                .entries
                                .insert(cache_key.clone(), CachedModelAnimEntry::Loading(h));
                        }

                        if let Some(CachedModelAnimEntry::Loading(gltf_h)) =
                            cache.entries.get(&cache_key).cloned()
                        {
                            if let Some(gltf) = gltf_assets.get(&gltf_h) {
                                let named_vec: Vec<(String, Handle<AnimationClip>)> = gltf
                                    .named_animations
                                    .iter()
                                    .map(|(k, v)| (k.as_ref().to_string(), v.clone()))
                                    .collect();
                                if let ConstructClipResolution::Ready {
                                    constructing_duration,
                                    articulated_buildup,
                                } = resolve_constructing_clip_duration(
                                    &named_vec,
                                    &vdef.animations,
                                    anim_clips.as_deref(),
                                ) {
                                    let next_entry = match build_animation_graph_from_clips(
                                        &named_vec,
                                        &gltf.animations,
                                        &vdef.animations,
                                    ) {
                                        Some((graph, state_nodes)) => CachedModelAnimEntry::Ready {
                                            graph_handle: graphs.add(graph),
                                            state_nodes,
                                            constructing_duration,
                                            articulated_buildup,
                                        },
                                        None => CachedModelAnimEntry::NoEmbeddedClips,
                                    };
                                    cache.entries.insert(cache_key.clone(), next_entry);
                                }
                            }
                        }

                        match cache.entries.get(&cache_key) {
                            Some(CachedModelAnimEntry::NoEmbeddedClips) => {
                                ctrl.anim_cache_resolved = true;
                                ctrl.articulated_buildup = false;
                                ctrl.was_scrubbing_construct = false;
                            }
                            Some(CachedModelAnimEntry::Ready {
                                graph_handle,
                                state_nodes,
                                constructing_duration,
                                articulated_buildup,
                            }) => {
                                ctrl.anim_cache_resolved = true;
                                ctrl.articulated_buildup = *articulated_buildup;
                                let scrub_progress = compute_building_scrub_progress(
                                    vis.building_footprint.is_some(),
                                    vis.spawn_progress,
                                    vis.deconstruct_progress,
                                );

                                if let (Some(p), Some(&construct_node), Some(dur)) = (
                                    scrub_progress,
                                    state_nodes.get(&EntityAnimState::Constructing),
                                    *constructing_duration,
                                ) {
                                    drove_gltf_clips = true;
                                    let seek_t = compute_construct_scrub_time(p, dur);
                                    let force_switch = ctrl.active_gltf_node
                                        != Some(construct_node)
                                        || !ctrl.was_scrubbing_construct;
                                    for &p_ent in &players {
                                        if let Ok((mut player, trans_opt, gh_opt)) =
                                            anim_player_q.get_mut(p_ent)
                                        {
                                            if let (Some(mut transitions), Some(_)) =
                                                (trans_opt, gh_opt)
                                            {
                                                apply_construct_scrub_to_player(
                                                    &mut player,
                                                    &mut transitions,
                                                    construct_node,
                                                    seek_t,
                                                    force_switch,
                                                );
                                            } else {
                                                let mut init_trans = AnimationTransitions::new();
                                                apply_construct_scrub_to_player(
                                                    &mut player,
                                                    &mut init_trans,
                                                    construct_node,
                                                    seek_t,
                                                    true,
                                                );
                                                commands.entity(p_ent).insert((
                                                    AnimationGraphHandle(graph_handle.clone()),
                                                    init_trans,
                                                ));
                                            }
                                        }
                                    }
                                    ctrl.active_gltf_node = Some(construct_node);
                                    ctrl.was_scrubbing_construct = true;
                                } else {
                                    let desired_state = if vis.building_footprint.is_some() {
                                        if under_construction
                                            && state_nodes
                                                .contains_key(&EntityAnimState::Constructing)
                                            && constructing_duration.is_none()
                                        {
                                            EntityAnimState::Constructing
                                        } else if ctrl.factory_rollout_timer > 0.0
                                            && state_nodes.contains_key(&EntityAnimState::Producing)
                                        {
                                            EntityAnimState::Producing
                                        } else if hp_ratio < 0.50
                                            && state_nodes.contains_key(&EntityAnimState::Damaged)
                                        {
                                            EntityAnimState::Damaged
                                        } else {
                                            ctrl.state
                                        }
                                    } else if hp_ratio < 0.35
                                        && ctrl.state == EntityAnimState::Idle
                                        && state_nodes.contains_key(&EntityAnimState::Damaged)
                                    {
                                        EntityAnimState::Damaged
                                    } else {
                                        ctrl.state
                                    };

                                    let resolved_node = state_nodes
                                        .get(&desired_state)
                                        .or_else(|| state_nodes.get(&EntityAnimState::Idle))
                                        .copied();

                                    if let Some(target_node) = resolved_node {
                                        drove_gltf_clips = true;
                                        let just_finished_scrub = ctrl.was_scrubbing_construct;
                                        let should_transition =
                                            ctrl.active_gltf_node != Some(target_node);
                                        let loops = anim_state_is_looping(desired_state);
                                        for &p_ent in &players {
                                            if let Ok((mut player, trans_opt, gh_opt)) =
                                                anim_player_q.get_mut(p_ent)
                                            {
                                                if just_finished_scrub {
                                                    if let (Some(&c_node), Some(dur)) = (
                                                        state_nodes
                                                            .get(&EntityAnimState::Constructing),
                                                        *constructing_duration,
                                                    ) {
                                                        snap_construct_to_rest_before_transition(
                                                            &mut player,
                                                            c_node,
                                                            dur,
                                                            target_node != c_node,
                                                        );
                                                    }
                                                }
                                                if let (Some(mut transitions), Some(_)) =
                                                    (trans_opt, gh_opt)
                                                {
                                                    if should_transition {
                                                        let active = transitions.play(
                                                            &mut player,
                                                            target_node,
                                                            Duration::from_millis(
                                                                GLTF_ANIM_CROSSFADE_MS,
                                                            ),
                                                        );
                                                        if loops {
                                                            active.repeat();
                                                        }
                                                    }
                                                } else {
                                                    let mut init_trans =
                                                        AnimationTransitions::new();
                                                    let active = init_trans.play(
                                                        &mut player,
                                                        target_node,
                                                        Duration::ZERO,
                                                    );
                                                    if loops {
                                                        active.repeat();
                                                    }
                                                    commands.entity(p_ent).insert((
                                                        AnimationGraphHandle(graph_handle.clone()),
                                                        init_trans,
                                                    ));
                                                }
                                            }
                                        }
                                        ctrl.active_gltf_node = Some(target_node);
                                    }
                                    ctrl.was_scrubbing_construct = false;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        ctrl.using_gltf_clips = drove_gltf_clips;

        let state = ctrl.state;
        let t = ctrl.state_elapsed + (vis.sim_id as f32 * 0.37);
        let door_open = ctrl.factory_door_open;
        let Some(rig) = ctrl.rig.clone() else {
            continue;
        };

        if let Some(scaf_ent) = rig.scaffold_frame {
            if let Ok((_, _, mut scaf_vis)) = node_q.get_mut(scaf_ent) {
                let want = if under_construction {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *scaf_vis != want {
                    *scaf_vis = want;
                }
            }
        }
        if !drove_gltf_clips {
            if let Some((door_ent, base_pos)) = rig.factory_door {
                if let Ok((_, mut door_tf, _)) = node_q.get_mut(door_ent) {
                    let target_pos = base_pos + Vec3::new(0.0, door_open * 0.72, 0.0);
                    let target_rot = Quat::from_rotation_x(-door_open * 1.18);
                    if door_tf.translation.distance_squared(target_pos) > 1e-8 {
                        door_tf.translation = target_pos;
                    }
                    if !door_tf.rotation.abs_diff_eq(target_rot, 1e-4)
                        && !door_tf.rotation.abs_diff_eq(-target_rot, 1e-4)
                    {
                        door_tf.rotation = target_rot;
                    }
                }
            }
        }
        if let Some(dmg_ent) = rig.damage_overlay {
            if let Ok((_, _, mut dmg_vis)) = node_q.get_mut(dmg_ent) {
                let want = if hp_ratio < 0.50 && vis.spawn_progress >= 0.99 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *dmg_vis != want {
                    *dmg_vis = want;
                }
            }
        }

        if drove_gltf_clips {
            continue;
        }

        let (torso_offset, torso_rot, head_rot, l_leg_rot, r_leg_rot, arm_rot) =
            evaluate_procedural_limb_pose(
                state,
                t,
                sim_ent,
                vis.death_anim_progress,
                ctrl.state_elapsed,
            );

        if let Some((e, base)) = rig.torso {
            if let Ok((_, mut tf, _)) = node_q.get_mut(e) {
                tf.translation = base + torso_offset;
                tf.rotation = torso_rot;
            }
        }
        if let Some((e, _)) = rig.head {
            if let Ok((_, mut tf, _)) = node_q.get_mut(e) {
                tf.rotation = head_rot;
            }
        }
        if let Some((e, _)) = rig.left_leg {
            if let Ok((_, mut tf, _)) = node_q.get_mut(e) {
                tf.rotation = l_leg_rot;
            }
        }
        if let Some((e, _)) = rig.right_leg {
            if let Ok((_, mut tf, _)) = node_q.get_mut(e) {
                tf.rotation = r_leg_rot;
            }
        }
        if let Some((e, _)) = rig.weapon_arm {
            if let Ok((_, mut tf, _)) = node_q.get_mut(e) {
                tf.rotation = arm_rot;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::{sync_sim_entities_to_3d, CachedFogVisibility, Ra2EntityVisualMode};
    use bevy::ecs::system::RunSystemOnce;
    use rts_sdk::loader::LoadedMod;
    use rts_simulation::world::SimWorld;

    #[test]
    fn mcv_conyard_unfold_folddown_and_articulated_no_y_squash_kinematics() {
        let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .unwrap();
        let loaded = LoadedMod::load_from_dir(mod_dir).unwrap();
        let asset_root = loaded.root_dir.parent().unwrap().to_path_buf();
        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        world.entities.clear();
        world.tick = 0;
        let map_cy = world
            .spawn_building(1, "allied_conyard", [8, 8], &loaded, false)
            .unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Scene>()
            .init_asset::<Image>()
            .init_resource::<CachedFogVisibility>()
            .init_resource::<crate::input::ClientInteractionState>()
            .insert_resource(Ra2EntityVisualMode::Full3dMeshes)
            .insert_resource(AssetRootResource(asset_root))
            .insert_resource(ModResource(loaded))
            .insert_resource(SimWorldResource(world.clone()));
        let sync = |app: &mut App| {
            let _ = app.world_mut().run_system_once(sync_sim_entities_to_3d);
            app.world_mut().flush();
        };
        sync(&mut app);

        world.tick = 20;
        let live_cy = {
            let mod_ref = &app.world().resource::<ModResource>().0;
            world
                .spawn_building(1, "soviet_conyard", [14, 14], mod_ref, false)
                .unwrap()
        };
        app.insert_resource(SimWorldResource(world.clone()));
        sync(&mut app);

        let mut q = app
            .world_mut()
            .query::<(Entity, &SimVisualEntity, &Transform)>();
        let mut cy_ent = Entity::PLACEHOLDER;
        for (e, vis, tf) in q.iter(app.world()) {
            if vis.sim_id == map_cy {
                assert!((vis.spawn_progress - 1.0).abs() < 1e-4);
            } else if vis.sim_id == live_cy {
                cy_ent = e;
                assert!((vis.spawn_progress - 0.04).abs() < 1e-4);
                assert!((tf.scale.y - tf.scale.x).abs() < 1e-4);
            }
        }

        app.world_mut()
            .entity_mut(cy_ent)
            .insert(EntityAnimController {
                anim_cache_resolved: true,
                ..Default::default()
            });
        sync(&mut app);
        let legacy_tf = app.world().entity(cy_ent).get::<Transform>().unwrap();
        assert!(legacy_tf.scale.y < legacy_tf.scale.x * 0.5);

        app.world_mut()
            .entity_mut(cy_ent)
            .get_mut::<EntityAnimController>()
            .unwrap()
            .articulated_buildup = true;
        world.entities.remove(&live_cy);
        app.insert_resource(SimWorldResource(world));
        sync(&mut app);
        let vis_after = app.world().entity(cy_ent).get::<SimVisualEntity>().unwrap();
        let tf_after = app.world().entity(cy_ent).get::<Transform>().unwrap();
        assert!(vis_after.deconstruct_progress.is_some());
        assert!((tf_after.scale.y - tf_after.scale.x).abs() < 1e-4);
    }
}
