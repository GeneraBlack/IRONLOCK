#[path = "shp_sprites_math.rs"]
mod shp_sprites_math;
pub use shp_sprites_math::*;

pub use rts_asset_pipeline::westwood::{
    infantry_facing_8dir, load_ra2_shp_sprite_catalog, select_building_sprite_frame,
    select_infantry_sprite_frame_flags, BuildingSpriteSpec, InfantrySpriteSpec,
};

use super::animation::select_unit_anim_state;
use super::preview_stage::{PreviewStageCamera, PreviewStageEntity};
#[cfg(test)]
use super::ArticulatedPartKind;
use super::{
    build_default_fow_overlay_image, resolve_accessible_team_color_in_mod, resolve_mod_asset_path,
    resolve_player_team_color_slot, ColorblindPaletteMode, ColorblindSettingsResource,
    FogOfWarOverlayImage, FogOfWarPlaneEntity, PlacementGhostModel, SimVisualEntity,
    SmoothFogTemporalState,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::pbr::{MaterialPlugin, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use rts_simulation::world::{SimEntity, SimWorld};
use shp_sprites_math::{resolve_sprite_shield_fx, SHP_SPRITE_WGSL};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::path::{Path, PathBuf};

#[derive(Component, Debug, Clone, Copy)]
pub struct SimGltfSceneChild {
    pub sim_id: u64,
}

#[derive(Component, Debug, Clone)]
pub struct Ra2HdSpriteQuad {
    pub sim_id: u64,
    pub entity_type_id: String,
    pub is_building: bool,
    pub active_frame: u32,
    pub min_world_y: f32,
    pub attack_hold_secs: f32,
    pub last_unit_yaw: f32,
    pub material_handle: Option<Handle<ShpSpriteMaterial>>,
}

#[derive(Resource, Default)]
pub struct Ra2SpriteGpuCache {
    pub quad_mesh: Option<Handle<Mesh>>,
    pub fallback_fow: Option<Handle<Image>>,
    pub atlas_handles: HashMap<String, Handle<Image>>,
}

pub fn toggle_ra2_entity_visual_mode_hotkey(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut visual_mode: ResMut<Ra2EntityVisualMode>,
) {
    if keys.as_deref().is_some_and(|k| k.just_pressed(KeyCode::F8)) {
        visual_mode.toggle();
    }
}

pub fn reload_ra2_sprite_catalog_if_needed(
    mod_res: Option<Res<ModResource>>,
    mut catalog: ResMut<Ra2ShpSpriteCatalog>,
    mut visual_mode: ResMut<Ra2EntityVisualMode>,
    mut gpu_cache: ResMut<Ra2SpriteGpuCache>,
) {
    let Some(mod_res) = mod_res else { return };
    let root = mod_res.0.root_dir.as_path();
    if catalog.loaded_mod_root.as_deref() == Some(root) {
        return;
    }
    gpu_cache.atlas_handles.clear();
    *visual_mode = Ra2EntityVisualMode::from_env_or_catalog(catalog.load_from_mod_root(root));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_ra2_hd_shp_sprites(
    mut commands: Commands,
    (time, asset_server, asset_root, mod_res, sim_res): (
        Option<Res<Time>>,
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
    ),
    (visual_mode, catalog, interaction, cb_settings, smooth_fow, fow_img_res): (
        Res<Ra2EntityVisualMode>,
        Res<Ra2ShpSpriteCatalog>,
        Option<Res<ClientInteractionState>>,
        Option<Res<ColorblindSettingsResource>>,
        Option<Res<SmoothFogTemporalState>>,
        Option<Res<FogOfWarOverlayImage>>,
    ),
    (mut gpu_cache, mut meshes, mut sprite_mats, mut images): (
        ResMut<Ra2SpriteGpuCache>,
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<ShpSpriteMaterial>>>,
        Option<ResMut<Assets<Image>>>,
    ),
    cam_q: Query<
        (&RtsCameraRig, &Transform),
        (
            Without<PreviewStageCamera>,
            Without<SimVisualEntity>,
            Without<SimGltfSceneChild>,
            Without<Ra2HdSpriteQuad>,
        ),
    >,
    visual_q: Query<
        (&SimVisualEntity, &Visibility),
        (Without<SimGltfSceneChild>, Without<Ra2HdSpriteQuad>),
    >,
    mut gltf_child_q: Query<
        (&SimGltfSceneChild, &mut Visibility),
        (
            Without<SimVisualEntity>,
            Without<Ra2HdSpriteQuad>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut sprite_quad_q: Query<
        (
            Entity,
            &mut Ra2HdSpriteQuad,
            &mut Transform,
            &mut Visibility,
        ),
        (
            Without<SimVisualEntity>,
            Without<SimGltfSceneChild>,
            Without<RtsCameraRig>,
            Without<PreviewStageCamera>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
) {
    let (Some(mod_res), Some(sim_res)) = (mod_res, sim_res) else {
        return;
    };
    let (world, loaded) = (&sim_res.0, &mod_res.0);
    let dt = time.as_deref().map(|t| t.delta_secs()).unwrap_or(0.016);
    let elapsed = time.as_deref().map(|t| t.elapsed_secs()).unwrap_or(0.0);
    let active_cam = cam_q.iter().next();
    let basis = CameraSpriteBasis::from_camera_transform(active_cam.map(|(_, tf)| tf));
    let strategic_fade = 1.0
        - active_cam
            .map(|(rig, _)| rig.strategic_zoom_blend())
            .unwrap_or(0.0);
    let mode = *visual_mode;

    let mut covered: HashMap<u64, (&SimVisualEntity, Visibility)> = HashMap::new();
    for (vis_ent, root_vis) in &visual_q {
        let is_b = vis_ent.building_footprint.is_some();
        if (is_b || vis_ent.is_infantry) && catalog.covers_entity(&vis_ent.entity_type_id, is_b) {
            covered.insert(vis_ent.sim_id, (vis_ent, *root_vis));
        }
    }
    for (child_tag, mut child_vis) in &mut gltf_child_q {
        let want = if mode == Ra2EntityVisualMode::HybridHdSprites
            && covered.contains_key(&child_tag.sim_id)
        {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *child_vis != want {
            *child_vis = want;
        }
    }
    if mode == Ra2EntityVisualMode::Full3dMeshes || !catalog.is_populated() {
        for (_ent, _quad, _tf, mut q_vis) in &mut sprite_quad_q {
            if *q_vis != Visibility::Hidden {
                *q_vis = Visibility::Hidden;
            }
        }
        return;
    }

    let quad_mesh_h = gpu_cache.quad_mesh.clone().or_else(|| {
        let h = meshes
            .as_mut()?
            .add(build_ground_anchored_sprite_quad_mesh());
        gpu_cache.quad_mesh = Some(h.clone());
        Some(h)
    });
    let fow_tex_h = fow_img_res.as_ref().map(|r| r.0.clone()).or_else(|| {
        gpu_cache.fallback_fow.clone().or_else(|| {
            let h = images.as_mut()?.add(build_default_fow_overlay_image(4, 4));
            gpu_cache.fallback_fow = Some(h.clone());
            Some(h)
        })
    });
    let cb_mode = cb_settings
        .as_deref()
        .map(|s| s.mode)
        .unwrap_or(ColorblindPaletteMode::Off);
    let local_pid = interaction
        .as_deref()
        .map(|i| i.local_player_id)
        .unwrap_or(1);
    let world_size = Vec2::new(
        (world.grid.width.max(1) as f32 * world.grid.tile_size).max(1.0),
        (world.grid.height.max(1) as f32 * world.grid.tile_size).max(1.0),
    );
    let fow_strength = smooth_fow
        .as_deref()
        .map(|s| s.fow_shader_strength)
        .unwrap_or(0.0);
    let mut synced_ids: HashSet<u64> = HashSet::with_capacity(covered.len());

    for (quad_ent, mut quad, mut tf, mut q_vis) in &mut sprite_quad_q {
        let Some(&(vis_ent, root_vis)) = covered.get(&quad.sim_id) else {
            commands.entity(quad_ent).despawn_recursive();
            continue;
        };
        if quad.entity_type_id != vis_ent.entity_type_id {
            commands.entity(quad_ent).despawn_recursive();
            continue;
        }
        synced_ids.insert(quad.sim_id);
        let sim_ent = world.entities.get(&quad.sim_id);
        if let Some(se) = sim_ent {
            if se.recoil_timer > 0.02 {
                quad.attack_hold_secs = 0.30;
            }
            quad.last_unit_yaw = se.facing_radians;
        }
        quad.attack_hold_secs = (quad.attack_hold_secs - dt).max(0.0);
        let want_vis = if root_vis == Visibility::Hidden || strategic_fade <= 0.005 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        if *q_vis != want_vis {
            *q_vis = want_vis;
        }
        if want_vis == Visibility::Hidden {
            continue;
        }

        if let Some((next_tf, uniform, frame_idx)) = evaluate_entity_sprite_pose(
            world,
            loaded,
            vis_ent,
            sim_ent,
            &catalog,
            &basis,
            quad.attack_hold_secs,
            quad.last_unit_yaw,
            local_pid,
            cb_mode,
            world_size,
            fow_strength,
            strategic_fade,
            elapsed,
        ) {
            quad.active_frame = frame_idx;
            quad.min_world_y = next_tf.translation.y;
            *tf = next_tf;
            if let (Some(mat_h), Some(ref mut mats)) = (&quad.material_handle, &mut sprite_mats) {
                if let Some(mat) = mats.get_mut(mat_h) {
                    if let Some(ref fh) = fow_tex_h {
                        if mat.fow_tex != *fh {
                            mat.fow_tex = fh.clone();
                        }
                    }
                    if mat.params != uniform {
                        mat.params = uniform;
                    }
                }
            }
        }
    }

    for (&sim_id, &(vis_ent, root_vis)) in &covered {
        if synced_ids.contains(&sim_id) {
            continue;
        }
        let sim_ent = world.entities.get(&sim_id);
        let init_yaw = sim_ent.map(|e| e.facing_radians).unwrap_or(0.0);
        let init_attack = if sim_ent.is_some_and(|e| e.recoil_timer > 0.02) {
            0.30
        } else {
            0.0
        };
        let is_building = vis_ent.building_footprint.is_some();
        let atlas_rel = if is_building {
            catalog
                .buildings
                .get(&vis_ent.entity_type_id)
                .map(|s| s.atlas.as_str())
        } else {
            catalog
                .infantry
                .get(&vis_ent.entity_type_id)
                .map(|s| s.atlas.as_str())
        };
        let Some(atlas_rel) = atlas_rel else { continue };
        let Some((init_tf, uniform, frame_idx)) = evaluate_entity_sprite_pose(
            world,
            loaded,
            vis_ent,
            sim_ent,
            &catalog,
            &basis,
            init_attack,
            init_yaw,
            local_pid,
            cb_mode,
            world_size,
            fow_strength,
            strategic_fade,
            elapsed,
        ) else {
            continue;
        };
        let init_vis = if root_vis == Visibility::Hidden || strategic_fade <= 0.005 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        let mat_handle = if let (Some(srv), Some(ref fow_h), Some(ref mut mats)) =
            (asset_server.as_deref(), &fow_tex_h, &mut sprite_mats)
        {
            let atlas_h = if let Some(existing) = gpu_cache.atlas_handles.get(atlas_rel) {
                existing.clone()
            } else {
                let resolved = asset_root.as_deref().map_or_else(
                    || atlas_rel.to_string(),
                    |ar| {
                        resolve_mod_asset_path(&ar.0, &loaded.root_dir, atlas_rel)
                            .trim_end_matches("#Scene0")
                            .to_string()
                    },
                );
                let loaded_h: Handle<Image> = srv.load(resolved);
                gpu_cache
                    .atlas_handles
                    .insert(atlas_rel.to_string(), loaded_h.clone());
                loaded_h
            };
            Some(mats.add(ShpSpriteMaterial {
                params: uniform,
                atlas_tex: atlas_h,
                fow_tex: fow_h.clone(),
            }))
        } else {
            None
        };
        let mut entity_cmds = commands.spawn((
            Ra2HdSpriteQuad {
                sim_id,
                entity_type_id: vis_ent.entity_type_id.clone(),
                is_building,
                active_frame: frame_idx,
                min_world_y: init_tf.translation.y,
                attack_hold_secs: init_attack,
                last_unit_yaw: init_yaw,
                material_handle: mat_handle.clone(),
            },
            init_tf,
            init_vis,
            NotShadowCaster,
            NotShadowReceiver,
        ));
        if let (Some(q_mesh), Some(m_h)) = (quad_mesh_h.clone(), mat_handle) {
            entity_cmds.insert((Mesh3d(q_mesh), MeshMaterial3d(m_h)));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_entity_sprite_pose(
    world: &SimWorld,
    loaded: &rts_sdk::loader::LoadedMod,
    vis_ent: &SimVisualEntity,
    sim_ent: Option<&SimEntity>,
    catalog: &Ra2ShpSpriteCatalog,
    basis: &CameraSpriteBasis,
    attack_hold_secs: f32,
    fallback_yaw: f32,
    local_pid: u8,
    cb_mode: ColorblindPaletteMode,
    world_size: Vec2,
    fow_strength: f32,
    strategic_fade: f32,
    elapsed: f32,
) -> Option<(Transform, ShpSpriteUniform, u32)> {
    let player_id = sim_ent.map(|e| e.player_id).unwrap_or(0);
    let slot = resolve_player_team_color_slot(world, loaded, player_id);
    let is_local = player_id != 0 && player_id == local_pid;
    let is_ally = player_id != 0 && (is_local || !world.are_hostile(local_pid.max(1), player_id));
    let team_rgba =
        resolve_accessible_team_color_in_mod(slot, is_local, is_ally, cb_mode, Some(loaded));
    let hp_ratio = sim_ent
        .map(|e| (e.health / e.max_health.max(1.0)).clamp(0.0, 1.0))
        .unwrap_or(1.0);
    let soot = if hp_ratio < 0.25 {
        0.75
    } else if hp_ratio < 0.50 {
        0.25
    } else {
        0.0
    };
    let (shield_strength, shield_col) =
        resolve_sprite_shield_fx(world, sim_ent).strength_and_color();
    let ground_y = sim_ent
        .map(|e| e.position[1])
        .unwrap_or(vis_ent.world_pos.y);
    let ground_pos = Vec3::new(vis_ent.world_pos.x, ground_y, vis_ent.world_pos.z);

    let (frame_idx, cols, cell_w, cell_h, team_remap, tf) = if vis_ent.building_footprint.is_some()
    {
        let s = catalog.buildings.get(&vis_ent.entity_type_id)?;
        let is_online = sim_ent.is_some_and(|e| world.is_building_online(e));
        let f_idx = select_building_sprite_frame(
            s,
            vis_ent.spawn_progress,
            vis_ent.deconstruct_progress,
            hp_ratio,
            is_online,
            elapsed,
        );
        let orig_sum = (s.orig_footprint[0] + s.orig_footprint[1]).max(1) as f32;
        let toml_fp = vis_ent.building_footprint.unwrap_or(s.toml_footprint);
        let m_per_px = (2.0 * std::f32::consts::SQRT_2 / 60.0)
            * ((toml_fp[0] + toml_fp[1]).max(1) as f32 / orig_sum);
        let tf = compute_ground_anchored_sprite_transform(
            ground_pos,
            s.crop_x,
            s.crop_y,
            s.crop_width,
            s.crop_height,
            s.foot_anchor_px,
            m_per_px,
            basis,
        );
        (
            f_idx,
            s.columns.max(1),
            s.cell_width,
            s.cell_height,
            s.team_remap,
            tf,
        )
    } else {
        let s = catalog.infantry.get(&vis_ent.entity_type_id)?;
        let unit_yaw = sim_ent.map(|e| e.facing_radians).unwrap_or(fallback_yaw);
        let facing_8 = infantry_facing_8dir(unit_yaw, basis.cam_yaw_rad);
        let anim_state = select_unit_anim_state(
            sim_ent,
            vis_ent.death_anim_progress.is_some(),
            attack_hold_secs,
        );
        let f_idx = select_infantry_sprite_frame(
            s,
            facing_8,
            anim_state,
            vis_ent.death_anim_progress,
            elapsed,
        );
        let m_per_px = s.world_height_m / s.body_height_px.max(12.0);
        let tf = compute_ground_anchored_sprite_transform(
            ground_pos,
            s.crop_x,
            s.crop_y,
            s.crop_width,
            s.crop_height,
            s.foot_anchor_px,
            m_per_px,
            basis,
        );
        (
            f_idx,
            s.columns.max(1),
            s.cell_width,
            s.cell_height,
            s.team_remap,
            tf,
        )
    };

    let (col, row) = (frame_idx % cols, frame_idx / cols);
    let uniform = ShpSpriteUniform {
        cell_rect_px: Vec4::new(
            (col * cell_w) as f32,
            (row * cell_h) as f32,
            cell_w as f32,
            cell_h as f32,
        ),
        team_color_srgb: Vec4::new(
            team_rgba[0],
            team_rgba[1],
            team_rgba[2],
            if team_remap && player_id != 0 {
                1.0
            } else {
                0.0
            },
        ),
        fx_params: Vec4::new(soot, shield_strength, strategic_fade, elapsed * 5.0),
        shield_color: Vec4::new(shield_col.x, shield_col.y, shield_col.z, 1.04),
        map_fow_params: Vec4::new(world_size.x, world_size.y, fow_strength, 0.0),
    };
    Some((tf, uniform, frame_idx))
}

pub fn register_ra2_shp_sprite_rendering(app: &mut App) {
    if let Some(mut shaders) = app.world_mut().get_resource_mut::<Assets<Shader>>() {
        shaders.insert(
            &SHP_SPRITE_SHADER_HANDLE,
            Shader::from_wgsl(SHP_SPRITE_WGSL, file!()),
        );
    }
    if app.is_plugin_added::<bevy::pbr::PbrPlugin>() {
        if !app.is_plugin_added::<MaterialPlugin<ShpSpriteMaterial>>() {
            app.add_plugins(MaterialPlugin::<ShpSpriteMaterial>::default());
        }
    } else if app.world().contains_resource::<AssetServer>()
        && !app.world().contains_resource::<Assets<ShpSpriteMaterial>>()
    {
        app.init_asset::<ShpSpriteMaterial>();
    }
    let mut catalog = Ra2ShpSpriteCatalog::default();
    let has_cat = app
        .world()
        .get_resource::<ModResource>()
        .is_some_and(|m| catalog.load_from_mod_root(&m.0.root_dir));
    let default_mode = Ra2EntityVisualMode::from_env_or_catalog(has_cat);
    app.insert_resource(catalog)
        .init_resource::<Ra2SpriteGpuCache>();
    if !app.world().contains_resource::<Ra2EntityVisualMode>() {
        app.insert_resource(default_mode);
    }
    app.add_systems(
        Update,
        (
            reload_ra2_sprite_catalog_if_needed,
            toggle_ra2_entity_visual_mode_hotkey,
            sync_ra2_hd_shp_sprites
                .after(super::sync_sim_entities_to_3d)
                .after(super::refresh_cached_fog_visibility)
                .after(super::step_smooth_fow_projection),
        )
            .chain(),
    );
}

#[cfg(test)]
#[path = "shp_sprites_tests.rs"]
mod tests;
