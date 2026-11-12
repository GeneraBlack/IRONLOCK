use super::history::{EditorCommand, TerrainStrokeState};
use super::spline_and_prefab_tools;
use super::world_builder::{apply_world_builder_brush_at_tile, WorldBuilderTool};
use super::{SdkInspectTarget, StudioState};
use crate::audio::SoundSettingsResource;
use crate::camera::RtsCameraRig;
use crate::input::{ClientInteractionState, UiAssetCache, UiInputBlocker};
use crate::rendering::{
    spawn_map_environment_entities, CachedFogVisibility, MapEnvironmentEntity, MapTerrainDirty,
    MapTerrainDirtyRects, PreviewStageCamera, SimVisualEntity,
};
use crate::vfx::{BattlefieldVfxEntity, VfxTrackerState};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::SimWorld;

/// Applies 3D viewport brush strokes, camera snap requests, outliner visibility masks,
/// and pending mod switches inside the open central 3D viewport.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_studio_viewport_and_mod_switch(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut camera_q: Query<
        (&Camera, &GlobalTransform, &mut RtsCameraRig, &mut Transform),
        (Without<SimVisualEntity>, Without<PreviewStageCamera>),
    >,
    mut vis_q: Query<(Entity, &SimVisualEntity, &mut Visibility)>,
    env_q: Query<Entity, With<MapEnvironmentEntity>>,
    vfx_q: Query<Entity, With<BattlefieldVfxEntity>>,
    (mut meshes, mut materials, mut vfx_state, mut cached_fow, mut sound_res): (
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
        ResMut<VfxTrackerState>,
        ResMut<CachedFogVisibility>,
        ResMut<SoundSettingsResource>,
    ),
    (
        mut studio_state,
        blocker,
        mut interaction,
        mut mod_res,
        mut sim_res,
        mut ui_cache,
        mut terrain_dirty,
        mut terrain_dirty_rects,
    ): (
        ResMut<StudioState>,
        Res<UiInputBlocker>,
        ResMut<ClientInteractionState>,
        ResMut<ModResource>,
        ResMut<SimWorldResource>,
        ResMut<UiAssetCache>,
        Option<ResMut<MapTerrainDirty>>,
        Option<ResMut<MapTerrainDirtyRects>>,
    ),
) {
    // Handle Pending Mod Switch requested from Studio File menu / Asset Browser
    if let Some(target_dir) = studio_state.pending_switch_mod_dir.take() {
        match LoadedMod::load_from_dir(&target_dir) {
            Ok(new_mod) => {
                for ent in &env_q {
                    commands.entity(ent).despawn_recursive();
                }
                for (ent, _, _) in &vis_q {
                    commands.entity(ent).despawn_recursive();
                }
                for ent in &vfx_q {
                    commands.entity(ent).despawn_recursive();
                }
                sim_res.0 = SimWorld::from_mod(&new_mod);
                vfx_state.reset();
                *cached_fow = CachedFogVisibility::default();
                sound_res.load_from_loaded_mod(&new_mod);
                ui_cache.clear_for_mod(&new_mod.manifest.id);
                studio_state.cameo_egui_textures.clear();
                studio_state.history.clear();
                studio_state.modified_units.clear();
                studio_state.modified_buildings.clear();
                studio_state.modified_weapons.clear();
                studio_state.modified_upgrades.clear();
                studio_state.modified_commander_trees.clear();
                studio_state.modified_factions.clear();
                studio_state.modified_resources.clear();
                studio_state.modified_ai_profiles.clear();
                studio_state.modified_campaigns.clear();
                studio_state.audio_modified = false;
                studio_state.modified_locales.clear();
                studio_state.pending_delete_confirm = None;
                studio_state.spline_tools.reload_prefabs(&new_mod);
                studio_state.validation_dirty = true;
                studio_state.inspect_target = new_mod
                    .units
                    .keys()
                    .next()
                    .cloned()
                    .map(SdkInspectTarget::Unit)
                    .unwrap_or(SdkInspectTarget::None);
                if let (Some(m), Some(mat)) = (meshes.as_deref_mut(), materials.as_deref_mut()) {
                    spawn_map_environment_entities(&mut commands, m, mat, &new_mod, &sim_res.0);
                }
                studio_state.status_banner = format!(
                    "Switched active mod to '{}' ({})",
                    new_mod.manifest.name, new_mod.manifest.id
                );
                mod_res.0 = new_mod;
                if let Some(d) = terrain_dirty.as_deref_mut() {
                    d.0 = true;
                }
                if let Some(r) = terrain_dirty_rects.as_deref_mut() {
                    r.mark_all();
                }
            }
            Err(err) => {
                studio_state.status_banner = format!("Failed loading mod {:?}: {err}", target_dir);
            }
        }
    }

    if !interaction.studio_open {
        return;
    }

    // Apply outliner visibility toggles onto 3D visual entities
    for (_, sim_vis, mut vis) in &mut vis_q {
        if studio_state.hidden_entity_ids.contains(&sim_vis.sim_id) {
            *vis = Visibility::Hidden;
        }
    }

    // Apply pending camera snap request from Scene Hierarchy / Trigger Studio / Previewer
    if let Some(target_world) = studio_state.camera_jump_request.take() {
        if let Ok((_, _, mut rig, mut tf)) = camera_q.get_single_mut() {
            rig.snap_to(target_world);
            *tf = rig.compute_transform();
        }
    }

    // Advance active Studio camera cutscene preview when running (`[▶ Preview Cutscene]`)
    if let Some((cs_id, elapsed)) = studio_state.spline_tools.active_cutscene_preview.clone() {
        let pose_opt = mod_res
            .0
            .entry_map
            .cutscenes
            .iter()
            .find(|c| c.id == cs_id)
            .and_then(|cs| spline_and_prefab_tools::sample_cutscene_camera_pose(cs, elapsed));
        match pose_opt {
            Some((focus, dist, pitch_deg, yaw_deg)) => {
                if let Ok((_, _, mut rig, mut tf)) = camera_q.get_single_mut() {
                    rig.cutscene_override = true;
                    rig.focus = focus;
                    rig.target_focus = focus;
                    rig.distance = dist;
                    rig.target_distance = dist;
                    rig.pitch_radians = pitch_deg.to_radians();
                    rig.target_pitch_radians = pitch_deg.to_radians();
                    rig.yaw_radians = yaw_deg.to_radians();
                    rig.target_yaw_radians = yaw_deg.to_radians();
                    *tf = rig.compute_transform();
                }
                studio_state.spline_tools.active_cutscene_preview =
                    Some((cs_id, elapsed + 1.0 / 60.0));
            }
            None => {
                if let Ok((_, _, mut rig, _)) = camera_q.get_single_mut() {
                    rig.cutscene_override = false;
                }
                studio_state.spline_tools.active_cutscene_preview = None;
                studio_state.status_banner = format!("⏹ Finished cutscene preview '{cs_id}'.");
            }
        }
    }

    // Handle 3D Viewport World Builder Brushes when not in live playtest
    if interaction.studio_playtest_active
        || studio_state.active_tool == WorldBuilderTool::SelectAndMove
        || studio_state.spline_tools.mode
            != spline_and_prefab_tools::ActiveAdvancedTerrainMode::None
    {
        studio_state.active_stroke_before = None;
        studio_state.last_painted_tile = None;
        return;
    }

    let cursor_opt = windows.get_single().ok().and_then(|w| w.cursor_position());
    if blocker.blocks_world_pointer(true, interaction.menu_open, cursor_opt) {
        if mouse_buttons.just_released(MouseButton::Left) {
            finalize_active_terrain_stroke(&mut studio_state, &sim_res.0, &mod_res.0);
        }
        return;
    }

    let Some(cursor) = cursor_opt else {
        return;
    };
    let Ok((camera, cam_gtf, _, _)) = camera_q.get_single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_gtf, cursor) else {
        return;
    };
    let dir_v3 = *ray.direction;
    let hit_opt = sim_res
        .0
        .grid
        .raycast_heightfield(ray.origin.into(), dir_v3.into(), 500.0)
        .map(Vec3::from)
        .or_else(|| {
            if dir_v3.y.abs() > 1e-5 {
                let t = -ray.origin.y / dir_v3.y;
                if t > 0.0 {
                    Some(ray.origin + dir_v3 * t)
                } else {
                    None
                }
            } else {
                None
            }
        });
    let Some(hit) = hit_opt else {
        return;
    };
    let tile = sim_res.0.grid.world_to_tile([hit.x, hit.y, hit.z]);
    interaction.hovered_tile = tile;

    let mut fallback_dirty = MapTerrainDirty(false);
    let dirty_ref: &mut MapTerrainDirty = match terrain_dirty.as_deref_mut() {
        Some(d) => d,
        None => &mut fallback_dirty,
    };

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if studio_state.active_tool.is_continuous_terrain_brush() {
            studio_state.active_stroke_before =
                Some(TerrainStrokeState::capture(&sim_res.0, &mod_res.0));
        }
        studio_state.last_painted_tile = Some(tile);
        let mut s_ref = studio_state;
        apply_world_builder_brush_at_tile(
            tile,
            true,
            interaction.placement_rotated,
            &mut s_ref,
            &mut mod_res.0,
            &mut sim_res.0,
            dirty_ref,
            terrain_dirty_rects.as_deref_mut(),
        );
    } else if mouse_buttons.pressed(MouseButton::Left)
        && studio_state.active_tool.is_continuous_terrain_brush()
        && studio_state.last_painted_tile != Some(tile)
    {
        studio_state.last_painted_tile = Some(tile);
        apply_world_builder_brush_at_tile(
            tile,
            false,
            interaction.placement_rotated,
            &mut studio_state,
            &mut mod_res.0,
            &mut sim_res.0,
            dirty_ref,
            terrain_dirty_rects.as_deref_mut(),
        );
    } else if mouse_buttons.just_released(MouseButton::Left) {
        finalize_active_terrain_stroke(&mut studio_state, &sim_res.0, &mod_res.0);
    }
}

fn finalize_active_terrain_stroke(state: &mut StudioState, world: &SimWorld, loaded: &LoadedMod) {
    state.last_painted_tile = None;
    if let Some(before) = state.active_stroke_before.take() {
        let after = TerrainStrokeState::capture(world, loaded);
        let label = format!("{} Stroke", state.active_tool.label());
        state.history.push_executed(EditorCommand::TerrainStroke {
            label,
            before: Box::new(before),
            after: Box::new(after),
        });
    }
}
