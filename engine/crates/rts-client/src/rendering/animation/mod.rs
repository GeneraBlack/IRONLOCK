mod clip_graph;

pub use clip_graph::{
    anim_state_is_looping, build_animation_graph_from_clips, resolve_clip_name_for_state,
    select_unit_anim_state, should_cull_entity_animation,
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
    (gltfs, mut anim_graphs, mut graph_cache): (
        Option<Res<Assets<Gltf>>>,
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
            ctrl.rig = Some(rig);
            ctrl.anim_players = Some(players);
        }
    }

    for (_ent, vis, root_tf, root_vis, ctrl_opt) in &mut root_q {
        let Some(mut ctrl) = ctrl_opt else {
            continue;
        };
        let is_dying = vis.death_anim_progress.is_some();
        let bound_r = vis
            .building_footprint
            .map(|[fw, fh]| (fw.max(fh) as f32) * 1.4)
            .unwrap_or(2.2);
        if !is_dying
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

        let under_construction = vis.spawn_progress < 0.992 || vis.deconstruct_progress.is_some();
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
                                let next_entry = match build_animation_graph_from_clips(
                                    &named_vec,
                                    &gltf.animations,
                                    &vdef.animations,
                                ) {
                                    Some((graph, state_nodes)) => CachedModelAnimEntry::Ready {
                                        graph_handle: graphs.add(graph),
                                        state_nodes,
                                    },
                                    None => CachedModelAnimEntry::NoEmbeddedClips,
                                };
                                cache.entries.insert(cache_key.clone(), next_entry);
                            }
                        }

                        if let Some(CachedModelAnimEntry::Ready {
                            graph_handle,
                            state_nodes,
                        }) = cache.entries.get(&cache_key)
                        {
                            let desired_state = if vis.building_footprint.is_some() {
                                if under_construction
                                    && state_nodes.contains_key(&EntityAnimState::Constructing)
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
                                let should_transition = ctrl.active_gltf_node != Some(target_node);
                                let loops = anim_state_is_looping(desired_state);
                                let mut all_ready = true;
                                for &p_ent in &players {
                                    if let Ok((mut player, trans_opt, gh_opt)) =
                                        anim_player_q.get_mut(p_ent)
                                    {
                                        if let (Some(mut transitions), Some(_)) =
                                            (trans_opt, gh_opt)
                                        {
                                            if should_transition {
                                                let active = transitions.play(
                                                    &mut player,
                                                    target_node,
                                                    Duration::from_millis(GLTF_ANIM_CROSSFADE_MS),
                                                );
                                                if loops {
                                                    active.repeat();
                                                }
                                            }
                                        } else {
                                            all_ready = false;
                                            commands.entity(p_ent).insert((
                                                AnimationGraphHandle(graph_handle.clone()),
                                                AnimationTransitions::new(),
                                            ));
                                        }
                                    }
                                }
                                if should_transition && all_ready {
                                    ctrl.active_gltf_node = Some(target_node);
                                }
                            }
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
    use rts_sdk::loader::LoadedMod;
    use rts_sdk::schema::common::AnimClipMap;
    use rts_simulation::world::SimWorld;

    #[test]
    fn anim_controller_transitions_across_idle_walk_prone_attack_and_die() {
        let mod_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../rust-ra2")
            .canonicalize()
            .expect("canonicalize rust-ra2 path");
        let loaded = LoadedMod::load_from_dir(mod_dir).expect("rust-ra2 should load");
        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
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
