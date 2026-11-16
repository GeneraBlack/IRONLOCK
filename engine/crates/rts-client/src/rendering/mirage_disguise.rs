use super::entity_articulation::{upgrade_scene_hierarchy_to_unit_pbr, ScenePbrUpgradeCtx};
use super::preview_stage::{PreviewStageCamera, PreviewStageEntity};
use super::{
    build_default_fow_overlay_image, build_procedural_micro_detail_image, resolve_mod_asset_path,
    CachedFogVisibility, DamageVisualTier, FogOfWarOverlayImage, MapEnvironmentEntity,
    MicroDetailTextureResource, Ra2TerrainTheater, ShieldFresnelKind, SimVisualEntity,
    SmoothFogTemporalState, UnitPbrTeamMaterial,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{TeamMaskMode, VisualDef};
use rts_simulation::world::{EntityKind, SimEntity, TileVisibility};
use std::collections::HashMap;

pub const TREE_PINE_REL_PATH: &str = "assets/models/props/tree_pine.glb";
pub const TREE_TEMPERATE_REL_PATH: &str = "assets/models/props/tree_temperate.glb";

#[derive(Component, Debug, Clone)]
pub struct MirageTreeDisguiseEntity {
    pub sim_id: u64,
    pub is_snow_pine: bool,
    pub is_hostile_pov: bool,
    pub meshes_upgraded: bool,
    pub material_handles: Vec<Handle<UnitPbrTeamMaterial>>,
}

#[derive(Resource, Default)]
pub struct MirageDisguiseCache {
    pub loaded_mod_root: Option<std::path::PathBuf>,
    pub temperate_tree_scene: Option<Handle<Scene>>,
    pub pine_tree_scene: Option<Handle<Scene>>,
    pub shimmer_ring_mesh: Option<Handle<Mesh>>,
    pub shimmer_ring_mat: Option<Handle<StandardMaterial>>,
    pub fire_hold_timers: HashMap<u64, f32>,
    pub prev_weapon_cooldowns: HashMap<u64, f32>,
}

#[inline]
pub fn select_mirage_tree_model_rel_path(theater: Ra2TerrainTheater) -> &'static str {
    if theater == Ra2TerrainTheater::Snow {
        TREE_PINE_REL_PATH
    } else {
        TREE_TEMPERATE_REL_PATH
    }
}

#[inline]
pub fn is_mirage_disguise_unit(loaded: &LoadedMod, unit_id: &str) -> bool {
    unit_id == "mirage_tank"
        || loaded
            .units
            .get(unit_id)
            .is_some_and(|u| u.stealth.disguises_as_terrain)
}

#[inline]
pub fn evaluate_mirage_disguise_active(sim_ent: &SimEntity, fire_hold_secs: f32) -> bool {
    sim_ent.health > 0.0
        && sim_ent.transported_by.is_none()
        && sim_ent.current_speed < 0.08
        && sim_ent.path_waypoints.is_empty()
        && sim_ent.recoil_timer <= 0.01
        && fire_hold_secs <= 0.001
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_mirage_tree_disguises(
    mut commands: Commands,
    (time, asset_server, asset_root, mod_res, sim_res, interaction): (
        Res<Time>,
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
        Option<Res<ClientInteractionState>>,
    ),
    (cached_fow, smooth_fow, fow_img_res, micro_detail_res): (
        Option<Res<CachedFogVisibility>>,
        Option<Res<SmoothFogTemporalState>>,
        Option<Res<FogOfWarOverlayImage>>,
        Option<Res<MicroDetailTextureResource>>,
    ),
    (mut cache, mut meshes, mut std_mats, mut unit_pbr_mats, mut images): (
        ResMut<MirageDisguiseCache>,
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
        Option<ResMut<Assets<UnitPbrTeamMaterial>>>,
        Option<ResMut<Assets<Image>>>,
    ),
    cam_rig_q: Query<&RtsCameraRig, Without<PreviewStageCamera>>,
    mut tank_vis_q: Query<
        (&SimVisualEntity, &Transform, &mut Visibility),
        (
            Without<MirageTreeDisguiseEntity>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut tree_q: Query<
        (
            Entity,
            &mut MirageTreeDisguiseEntity,
            &mut Transform,
            &mut Visibility,
        ),
        (Without<SimVisualEntity>, Without<PreviewStageEntity>),
    >,
    children_q: Query<&Children>,
    node_q: Query<
        (
            Option<&Name>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        Without<PreviewStageEntity>,
    >,
) {
    let (Some(mod_res), Some(sim_res), Some(cached_fow), Some(srv)) =
        (mod_res, sim_res, cached_fow, asset_server.as_deref())
    else {
        return;
    };
    let (world, loaded, dt, elapsed) = (
        &sim_res.0,
        &mod_res.0,
        time.delta_secs(),
        time.elapsed_secs(),
    );

    if let (Some(ma), Some(sma)) = (meshes.as_deref_mut(), std_mats.as_deref_mut()) {
        if cache.shimmer_ring_mesh.is_none() {
            cache.shimmer_ring_mesh = Some(ma.add(Torus::new(0.88, 0.96)));
            cache.shimmer_ring_mat = Some(sma.add(StandardMaterial {
                base_color: Color::srgba(0.24, 0.98, 0.72, 0.65),
                emissive: LinearRgba::new(0.35, 2.2, 1.45, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }));
        }
    }

    if cache.loaded_mod_root.as_deref() != Some(loaded.root_dir.as_path()) {
        let load_tree = |rel: &str| -> Option<Handle<Scene>> {
            loaded.asset_exists(rel).then(|| {
                let p = asset_root
                    .as_deref()
                    .map(|ar| resolve_mod_asset_path(&ar.0, &loaded.root_dir, rel))
                    .unwrap_or_else(|| format!("{rel}#Scene0"));
                srv.load(p)
            })
        };
        cache.temperate_tree_scene = load_tree(TREE_TEMPERATE_REL_PATH);
        cache.pine_tree_scene = load_tree(TREE_PINE_REL_PATH);
        cache.fire_hold_timers.clear();
        cache.prev_weapon_cooldowns.clear();
        cache.loaded_mod_root = Some(loaded.root_dir.clone());
    }

    let local_pid = interaction
        .as_deref()
        .map(|i| CachedFogVisibility::effective_pov_and_fog(world, i).0)
        .unwrap_or(cached_fow.pov_player_id.max(1));
    let is_snow_pine =
        Ra2TerrainTheater::infer_from_map(&loaded.entry_map.id, &loaded.entry_map.name)
            == Ra2TerrainTheater::Snow;
    let strategic_fade = 1.0
        - cam_rig_q
            .iter()
            .next()
            .map(|r| r.strategic_zoom_blend())
            .unwrap_or(0.0);

    cache
        .fire_hold_timers
        .retain(|id, _| world.entities.contains_key(id));
    cache
        .prev_weapon_cooldowns
        .retain(|id, _| world.entities.contains_key(id));

    let mut desired: HashMap<u64, (Vec3, bool, bool)> = HashMap::new();
    for (vis, tank_tf, mut tank_vis) in &mut tank_vis_q {
        let Some(sim_ent) = world.entities.get(&vis.sim_id) else {
            continue;
        };
        let EntityKind::Unit { ref unit_id } = sim_ent.kind else {
            continue;
        };
        if !is_mirage_disguise_unit(loaded, unit_id) {
            continue;
        }
        let prev_cd = cache
            .prev_weapon_cooldowns
            .insert(vis.sim_id, sim_ent.weapon_cooldown)
            .unwrap_or(sim_ent.weapon_cooldown);
        let hold = cache.fire_hold_timers.entry(vis.sim_id).or_insert(0.0);
        if sim_ent.recoil_timer > 0.01
            || sim_ent.weapon_cooldown > prev_cd + 0.05
            || sim_ent.reveal_timer > 1.40
        {
            *hold = 0.60;
        } else {
            *hold = (*hold - dt).max(0.0);
        }
        if !evaluate_mirage_disguise_active(sim_ent, *hold) {
            continue;
        }
        let is_hostile = sim_ent.player_id != 0 && world.are_hostile(local_pid, sim_ent.player_id);
        if is_hostile && cached_fow.fog_enabled && cached_fow.is_detected_by_pov(sim_ent.position) {
            continue;
        }
        if is_hostile {
            *tank_vis = Visibility::Hidden;
            let tile_vis = !cached_fow.fog_enabled
                || cached_fow.world_pos_visibility(world, sim_ent.position)
                    != TileVisibility::Unexplored;
            desired.insert(vis.sim_id, (tank_tf.translation, true, tile_vis));
        } else {
            desired.insert(
                vis.sim_id,
                (tank_tf.translation, false, *tank_vis != Visibility::Hidden),
            );
        }
    }

    let fow_tex = fow_img_res.map(|r| r.0.clone()).unwrap_or_else(|| {
        images.as_mut().map_or_else(Handle::default, |im| {
            let h = im.add(build_default_fow_overlay_image(64, 64));
            commands.insert_resource(FogOfWarOverlayImage(h.clone()));
            h
        })
    });
    let micro_tex = micro_detail_res.map(|r| r.0.clone()).unwrap_or_else(|| {
        images.as_mut().map_or_else(Handle::default, |im| {
            let h = im.add(build_procedural_micro_detail_image());
            commands.insert_resource(MicroDetailTextureResource(h.clone()));
            h
        })
    });
    let mut pbr_ctx = ScenePbrUpgradeCtx {
        std_mats: std_mats.as_deref(),
        ext_mats: unit_pbr_mats.as_deref_mut(),
        fow_tex,
        micro_tex,
        world_size: Vec2::new(
            (world.grid.width.max(1) as f32 * world.grid.tile_size).max(1.0),
            (world.grid.height.max(1) as f32 * world.grid.tile_size).max(1.0),
        ),
        fow_strength: smooth_fow
            .as_deref()
            .map(|s| s.fow_shader_strength)
            .unwrap_or(if cached_fow.fog_enabled { 1.0 } else { 0.0 }),
        strategic_fade,
        elapsed,
    };
    let tree_vdef = VisualDef {
        team_color_tint: false,
        team_mask_mode: TeamMaskMode::Disabled,
        ..Default::default()
    };

    for (ent, mut tree_vis, mut tf, mut vis) in &mut tree_q {
        let Some((base_pos, is_hostile, tile_vis)) = desired.remove(&tree_vis.sim_id) else {
            commands.entity(ent).despawn_recursive();
            continue;
        };
        if tree_vis.is_snow_pine != is_snow_pine || tree_vis.is_hostile_pov != is_hostile {
            commands.entity(ent).despawn_recursive();
            desired.insert(tree_vis.sim_id, (base_pos, is_hostile, tile_vis));
            continue;
        }
        if is_hostile {
            tf.translation = base_pos;
            tf.rotation = Quat::from_rotation_y(
                (tree_vis.sim_id as f32 * 1.618).rem_euclid(std::f32::consts::TAU),
            );
            tf.scale = Vec3::ONE;
        } else {
            let bob = (elapsed * 2.2 + tree_vis.sim_id as f32).sin() * 0.06;
            tf.translation = base_pos + Vec3::new(-0.58, 0.30 + bob, -0.44);
            tf.rotation = Quat::from_rotation_y(elapsed * 0.55 + tree_vis.sim_id as f32);
            tf.scale = Vec3::splat(0.54);
        }
        *vis = if tile_vis && strategic_fade > 0.01 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };

        if !tree_vis.meshes_upgraded {
            let shield = if is_hostile {
                ShieldFresnelKind::None
            } else {
                ShieldFresnelKind::ChronoTemporal
            };
            if let Some(handles) = upgrade_scene_hierarchy_to_unit_pbr(
                &mut commands,
                ent,
                true,
                &children_q,
                &node_q,
                &mut pbr_ctx,
                &tree_vdef,
                [0.80, 0.84, 0.80, 1.0],
                DamageVisualTier::Intact,
                shield,
            ) {
                tree_vis.material_handles = handles;
                tree_vis.meshes_upgraded = true;
            }
        } else {
            pbr_ctx.refresh_handles(&tree_vis.material_handles);
        }
    }

    let scene_opt = if is_snow_pine {
        cache
            .pine_tree_scene
            .clone()
            .or_else(|| cache.temperate_tree_scene.clone())
    } else {
        cache
            .temperate_tree_scene
            .clone()
            .or_else(|| cache.pine_tree_scene.clone())
    };
    if let Some(scene_h) = scene_opt {
        for (sim_id, (base_pos, is_hostile, tile_vis)) in desired {
            let init_tf = if is_hostile {
                Transform::from_translation(base_pos).with_rotation(Quat::from_rotation_y(
                    (sim_id as f32 * 1.618).rem_euclid(std::f32::consts::TAU),
                ))
            } else {
                Transform::from_translation(base_pos + Vec3::new(-0.58, 0.30, -0.44))
                    .with_scale(Vec3::splat(0.54))
            };
            let (rm_opt, rmat_opt) = (
                cache.shimmer_ring_mesh.clone(),
                cache.shimmer_ring_mat.clone(),
            );
            commands
                .spawn((
                    MapEnvironmentEntity,
                    MirageTreeDisguiseEntity {
                        sim_id,
                        is_snow_pine,
                        is_hostile_pov: is_hostile,
                        meshes_upgraded: false,
                        material_handles: Vec::new(),
                    },
                    SceneRoot(scene_h.clone()),
                    if tile_vis {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    init_tf,
                ))
                .with_children(|parent| {
                    if !is_hostile {
                        if let (Some(rm), Some(rmat)) = (rm_opt, rmat_opt) {
                            parent.spawn((
                                Mesh3d(rm),
                                MeshMaterial3d(rmat),
                                bevy::pbr::NotShadowCaster,
                                Transform::from_xyz(1.07, -0.50, 0.81)
                                    .with_scale(Vec3::new(1.85, 0.18, 1.85)),
                            ));
                        }
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_simulation::world::SimWorld;

    #[test]
    fn mirage_disguise_checks_stationary_firing_hold_and_snow_vs_temperate_tree() {
        assert_eq!(
            select_mirage_tree_model_rel_path(Ra2TerrainTheater::Snow),
            TREE_PINE_REL_PATH
        );
        assert_eq!(
            select_mirage_tree_model_rel_path(Ra2TerrainTheater::Temperate),
            TREE_TEMPERATE_REL_PATH
        );
        let loaded = LoadedMod::load_from_dir(LoadedMod::resolve_mod_dir("rust-ra2")).unwrap();
        assert!(is_mirage_disguise_unit(&loaded, "mirage_tank"));
        assert!(!is_mirage_disguise_unit(&loaded, "grizzly_tank"));

        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        let mid = world
            .spawn_unit(2, "mirage_tank", [10, 10], &loaded)
            .unwrap();
        let ent = world.entities.get_mut(&mid).unwrap();
        ent.current_speed = 0.0;
        ent.path_waypoints.clear();
        assert!(evaluate_mirage_disguise_active(ent, 0.0));
        assert!(!evaluate_mirage_disguise_active(ent, 0.45));
        ent.current_speed = 2.5;
        assert!(!evaluate_mirage_disguise_active(ent, 0.0));
    }
}
