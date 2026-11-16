mod micro_detail_tex;
mod palette;
mod pbr_shader_ext;

pub use micro_detail_tex::*;
pub use palette::*;
pub use pbr_shader_ext::*;

use super::preview_stage::{PreviewStageCamera, PreviewStageEntity};
use super::{
    build_default_fow_overlay_image, resolve_mod_asset_path, CachedFogVisibility,
    FogOfWarOverlayImage, MaterialPreviewOverrideResource, PlacementGhostModel, SimVisualEntity,
    SmoothFogTemporalState,
};
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::VisualDef;
use rts_simulation::world::{EntityKind, SimWorld};

/// Cached per-entity glTF mesh descendants + original `StandardMaterial` handles so glTF textures
/// (`base_color_texture`, `normal_map_texture`, `metallic_roughness_texture`, `emissive_texture`)
/// are preserved 100% when upgrading to `UnitPbrTeamMaterial` (`Task 236`).
#[derive(Component, Debug, Clone, Default)]
pub struct SyncedGltfSceneVisuals {
    pub mesh_entities: Vec<(Entity, bool)>,
    pub orig_std_materials: Vec<Option<Handle<StandardMaterial>>>,
    pub lod_configured: bool,
    pub applied_material_key: Option<(u8, DamageVisualTier)>,
    pub applied_pbr_key: Option<u64>,
    pub active_construct_handles: Vec<Handle<UnitPbrTeamMaterial>>,
    pub cached_owner_state: Option<(u8, u8, bool, f32)>,
    pub cached_damage_tier: DamageVisualTier,
}

/// Tracks original `StandardMaterial` on Studio 3D Preview Stage (`RenderLayers::layer(3)`) meshes
/// so `MaterialPreviewOverrideResource` edits apply live without losing glTF textures (`Task 245`).
#[derive(Component, Debug, Clone)]
pub struct PreviewMeshOriginalMaterial {
    pub orig_std: Option<Handle<StandardMaterial>>,
    pub applied_hash: u64,
}

#[allow(clippy::type_complexity)]
fn discover_gltf_scene_meshes(
    root: Entity,
    children_q: &Query<&Children>,
    mesh_node_q: &Query<
        (
            Option<&Name>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        (
            Without<SimVisualEntity>,
            Without<super::FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
) -> Option<(Vec<(Entity, bool)>, Vec<Option<Handle<StandardMaterial>>>)> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let mut meshes = Vec::new();
    let mut orig_mats = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((curr, parent_is_lod1)) = stack.pop() {
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                let mut is_lod1 = parent_is_lod1;
                if let Ok((name_opt, mesh_opt, mat_opt)) = mesh_node_q.get(child) {
                    if let Some(name) = name_opt {
                        if is_lod1_node_name(name.as_str()) {
                            is_lod1 = true;
                        }
                    }
                    if mesh_opt.is_some() {
                        meshes.push((child, is_lod1));
                        orig_mats.push(mat_opt.map(|m| m.0.clone()));
                    }
                }
                stack.push((child, is_lod1));
            }
        }
    }
    if meshes.is_empty() {
        None
    } else {
        Some((meshes, orig_mats))
    }
}

fn resolve_entity_shield_fresnel(
    world: &SimWorld,
    sim_ent: &rts_simulation::world::SimEntity,
) -> ShieldFresnelKind {
    if sim_ent.invulnerable_remaining() > 0.0 {
        ShieldFresnelKind::IronCurtain
    } else if sim_ent.temporal_progress > 0.01
        || world.trait_stores.temporal.contains_key(&sim_ent.id)
        || sim_ent
            .active_buffs
            .iter()
            .any(|b| b.phased_out && b.remaining_secs > 0.0)
    {
        ShieldFresnelKind::ChronoTemporal
    } else if world.trait_stores.psionic.contains_key(&sim_ent.id) || sim_ent.is_confused() {
        ShieldFresnelKind::PsionicControl
    } else if sim_ent.has_power_charge_boost() {
        ShieldFresnelKind::TeslaOvercharge
    } else {
        ShieldFresnelKind::None
    }
}

fn load_optional_mod_image(
    asset_server: Option<&AssetServer>,
    asset_root: Option<&AssetRootResource>,
    loaded: &LoadedMod,
    opt_path: Option<&str>,
) -> Option<Handle<Image>> {
    let raw = opt_path?.trim();
    if raw.is_empty() {
        return None;
    }
    let server = asset_server?;
    let clean = raw.trim_start_matches('/').replace('\\', "/");
    let resolved = if let Some(ar) = asset_root {
        resolve_mod_asset_path(&ar.0, &loaded.root_dir, &clean)
            .trim_end_matches("#Scene0")
            .to_string()
    } else {
        clean
    };
    Some(server.load(resolved))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_gltf_lod_and_team_damage_materials(
    mut commands: Commands,
    (time, asset_server, asset_root, mod_res, sim_res): (
        Option<Res<Time>>,
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
    ),
    (interaction, cb_settings, preview_ovr, cached_fow, smooth_fow, fow_img_res, micro_detail_res): (
        Option<Res<crate::input::ClientInteractionState>>,
        Option<Res<ColorblindSettingsResource>>,
        Option<Res<MaterialPreviewOverrideResource>>,
        Option<Res<CachedFogVisibility>>,
        Option<Res<SmoothFogTemporalState>>,
        Option<Res<FogOfWarOverlayImage>>,
        Option<Res<MicroDetailTextureResource>>,
    ),
    cam_rig_q: Query<
        &crate::camera::RtsCameraRig,
        (
            Without<PreviewStageCamera>,
            Without<SimVisualEntity>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut palette: ResMut<TeamDamageMaterialPalette>,
    mut pbr_cache: Option<ResMut<UnitPbrTeamMaterialCache>>,
    (materials, mut unit_pbr_mats, mut images): (
        Option<ResMut<Assets<StandardMaterial>>>,
        Option<ResMut<Assets<UnitPbrTeamMaterial>>>,
        Option<ResMut<Assets<Image>>>,
    ),
    mut root_q: Query<
        (
            Entity,
            &SimVisualEntity,
            Option<&mut SyncedGltfSceneVisuals>,
        ),
        Without<PreviewStageEntity>,
    >,
    children_q: Query<&Children>,
    mesh_node_q: Query<
        (
            Option<&Name>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        (
            Without<SimVisualEntity>,
            Without<super::FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut preview_mesh_q: Query<
        (
            Entity,
            &Mesh3d,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&mut PreviewMeshOriginalMaterial>,
            Option<&Name>,
        ),
        (
            With<PreviewStageEntity>,
            Without<SimVisualEntity>,
            Without<PreviewStageCamera>,
        ),
    >,
) {
    let (Some(mod_res), Some(sim_res), Some(mut mats)) = (mod_res, sim_res, materials) else {
        return;
    };
    let world = &sim_res.0;
    let loaded = &mod_res.0;
    let elapsed_secs = time.as_deref().map(|t| t.elapsed_secs()).unwrap_or(0.0);
    let cb_mode = cb_settings
        .as_deref()
        .map(|s| s.mode)
        .unwrap_or(ColorblindPaletteMode::Off);
    let local_pid = interaction
        .as_deref()
        .map(|i| i.local_player_id)
        .unwrap_or(1);
    let strategic_fade = 1.0
        - cam_rig_q
            .iter()
            .next()
            .map(|r| r.strategic_zoom_blend())
            .unwrap_or(0.0);
    let world_size = Vec2::new(
        (world.grid.width.max(1) as f32 * world.grid.tile_size).max(1.0),
        (world.grid.height.max(1) as f32 * world.grid.tile_size).max(1.0),
    );
    let fow_strength = smooth_fow
        .as_deref()
        .map(|s| s.fow_shader_strength)
        .unwrap_or_else(|| {
            if cached_fow.as_deref().is_some_and(|c| c.fog_enabled) {
                1.0
            } else {
                0.0
            }
        });

    let fow_tex_handle = if let Some(res) = fow_img_res {
        res.0.clone()
    } else if let Some(ref mut imgs) = images {
        let h_img = imgs.add(build_default_fow_overlay_image(
            world.grid.width.max(1) as u32,
            world.grid.height.max(1) as u32,
        ));
        commands.insert_resource(FogOfWarOverlayImage(h_img.clone()));
        h_img
    } else {
        Handle::default()
    };

    let micro_tex_handle = if let Some(res) = micro_detail_res {
        res.0.clone()
    } else if let Some(ref mut imgs) = images {
        let h_micro = imgs.add(build_procedural_micro_detail_image());
        commands.insert_resource(MicroDetailTextureResource(h_micro.clone()));
        h_micro
    } else {
        Handle::default()
    };

    // Pass 1: discover newly spawned glTF scene mesh nodes + preserve their original `Handle<StandardMaterial>`
    let mut newly_discovered = Vec::new();
    for (ent, _vis, synced_opt) in &root_q {
        match synced_opt {
            None => {
                commands
                    .entity(ent)
                    .insert(SyncedGltfSceneVisuals::default());
            }
            Some(synced) if !synced.lod_configured => {
                if let Some((found, orig_mats)) =
                    discover_gltf_scene_meshes(ent, &children_q, &mesh_node_q)
                {
                    newly_discovered.push((ent, found, orig_mats));
                }
            }
            _ => {}
        }
    }

    for (ent, found, orig_mats) in newly_discovered {
        if let Ok((_, _, Some(mut synced))) = root_q.get_mut(ent) {
            for &(mesh_ent, is_lod1) in &found {
                let range = if is_lod1 {
                    lod1_visibility_range()
                } else {
                    lod0_visibility_range()
                };
                commands.entity(mesh_ent).insert(range);
            }
            synced.mesh_entities = found;
            synced.orig_std_materials = orig_mats;
            synced.lod_configured = true;
            synced.applied_material_key = None;
            synced.applied_pbr_key = None;
            synced.active_construct_handles.clear();
        }
    }

    // Pass 2: bind `UnitPbrTeamMaterial` (`unit_pbr_team.wgsl`, preserving glTF PBR textures & COLOR_0)
    // when `Assets<UnitPbrTeamMaterial>` is active, falling back to `TeamDamageMaterialPalette` in minimal tests.
    if let (Some(ref mut ext_mats), Some(ref mut cache)) =
        (unit_pbr_mats.as_deref_mut(), pbr_cache.as_deref_mut())
    {
        let default_std = StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.56,
            metallic: 0.28,
            ..default()
        };

        for (_ent, vis, synced_opt) in &mut root_q {
            let Some(mut synced) = synced_opt else {
                continue;
            };
            if !synced.lod_configured || synced.mesh_entities.is_empty() {
                continue;
            }
            let is_building = vis.building_footprint.is_some();
            let is_deconstructing = is_building && vis.deconstruct_progress.is_some();
            let is_constructing = is_building && !is_deconstructing && vis.spawn_progress < 0.999;
            let needs_dynamic_construct_mat = is_constructing || is_deconstructing;
            let b_def = loaded.buildings.get(&vis.entity_type_id);

            let (slot, is_local, is_ally, tier, shield, visual_def, base_weld_code) =
                if let Some(sim_ent) = world.entities.get(&vis.sim_id) {
                    let s = resolve_player_team_color_slot(world, loaded, sim_ent.player_id);
                    let loc = sim_ent.player_id == local_pid;
                    let ally =
                        sim_ent.player_id != 0 && !world.are_hostile(local_pid, sim_ent.player_id);
                    let hp_ratio = sim_ent.health / sim_ent.max_health.max(1.0);
                    let d_tier = DamageVisualTier::from_hp_ratio(hp_ratio);
                    let sh = resolve_entity_shield_fresnel(world, sim_ent);
                    let v_def: Option<&VisualDef> = match &sim_ent.kind {
                        EntityKind::Unit { unit_id } => {
                            loaded.units.get(unit_id).map(|u| &u.visual)
                        }
                        EntityKind::Building { building_id } => {
                            loaded.buildings.get(building_id).map(|b| &b.visual)
                        }
                    };
                    let w_code = resolve_building_weld_code(world, sim_ent.player_id, b_def);
                    synced.cached_owner_state = Some((sim_ent.player_id, s, ally, w_code));
                    synced.cached_damage_tier = d_tier;
                    (s, loc, ally, d_tier, sh, v_def, w_code)
                } else if is_deconstructing {
                    let fallback_fac = b_def.map(|b| b.faction.as_str()).unwrap_or("allies");
                    let (pid, s, ally, w_code) = synced.cached_owner_state.unwrap_or_else(|| {
                        (
                            0,
                            default_faction_slot(fallback_fac),
                            false,
                            resolve_faction_weld_code(fallback_fac),
                        )
                    });
                    (
                        s,
                        pid != 0 && pid == local_pid,
                        ally,
                        synced.cached_damage_tier,
                        ShieldFresnelKind::None,
                        b_def.map(|b| &b.visual),
                        w_code,
                    )
                } else {
                    continue;
                };

            let team_rgba = resolve_accessible_team_color_in_mod(
                slot,
                is_local,
                is_ally,
                cb_mode,
                Some(loaded),
            );
            let q_rgba = [
                (team_rgba[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (team_rgba[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (team_rgba[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            ];
            let v_hash = hash_visual_pbr_spec(visual_def);

            let state_sig = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                q_rgba.hash(&mut h);
                (slot == 0).hash(&mut h);
                tier.hash(&mut h);
                shield.hash(&mut h);
                v_hash.hash(&mut h);
                h.finish()
            };

            if needs_dynamic_construct_mat {
                let c_params = compute_building_construction_params(
                    vis.world_pos.y,
                    vis.spawn_progress,
                    vis.deconstruct_progress,
                    vis.building_footprint,
                    b_def,
                    base_weld_code,
                );
                let rebuild_dyn = synced.active_construct_handles.len()
                    != synced.mesh_entities.len()
                    || synced.applied_pbr_key != Some(state_sig);
                if rebuild_dyn {
                    let custom_norm = load_optional_mod_image(
                        asset_server.as_deref(),
                        asset_root.as_deref(),
                        loaded,
                        visual_def.and_then(|v| v.normal_map_path.as_deref()),
                    );
                    let custom_orm = load_optional_mod_image(
                        asset_server.as_deref(),
                        asset_root.as_deref(),
                        loaded,
                        visual_def.and_then(|v| v.orm_map_path.as_deref()),
                    );
                    let mesh_list = synced.mesh_entities.clone();
                    let orig_list = synced.orig_std_materials.clone();
                    synced.active_construct_handles.clear();
                    for (idx, (mesh_ent, _)) in mesh_list.into_iter().enumerate() {
                        let orig_h = orig_list.get(idx).and_then(|o| o.as_ref());
                        let base_ref = orig_h.and_then(|h| mats.get(h)).unwrap_or(&default_std);
                        let mut mat_val = build_unit_pbr_team_material(
                            base_ref,
                            visual_def,
                            team_rgba,
                            slot == 0,
                            tier,
                            shield,
                            custom_norm.clone(),
                            custom_orm.clone(),
                            fow_tex_handle.clone(),
                            world_size,
                            fow_strength,
                            strategic_fade,
                            elapsed_secs,
                        );
                        mat_val.extension.micro_detail_tex = micro_tex_handle.clone();
                        mat_val.extension.params.construction_params = c_params;
                        let h_dyn = ext_mats.add(mat_val);
                        synced.active_construct_handles.push(h_dyn.clone());
                        commands
                            .entity(mesh_ent)
                            .remove::<MeshMaterial3d<StandardMaterial>>()
                            .insert(MeshMaterial3d(h_dyn));
                    }
                    synced.applied_pbr_key = Some(state_sig);
                    synced.applied_material_key = Some((slot, tier));
                } else {
                    for h_dyn in &synced.active_construct_handles {
                        if let Some(mat) = ext_mats.get_mut(h_dyn) {
                            mat.extension.params.construction_params = c_params;
                            mat.extension.params.damage_and_fx.w = strategic_fade;
                            mat.extension.params.map_fow_params =
                                Vec4::new(world_size.x, world_size.y, fow_strength, elapsed_secs);
                            if fow_tex_handle != Handle::default()
                                && mat.extension.fow_tex != fow_tex_handle
                            {
                                mat.extension.fow_tex = fow_tex_handle.clone();
                            }
                            if micro_tex_handle != Handle::default()
                                && mat.extension.micro_detail_tex != micro_tex_handle
                            {
                                mat.extension.micro_detail_tex = micro_tex_handle.clone();
                            }
                        }
                    }
                }
                continue;
            }

            // Finished buildings (`spawn_progress >= 0.999 && deconstruct_progress.is_none()`) and all units
            // revert cleanly to 100% shared `UnitPbrTeamMaterialCache` handles (`0` per-entity overhead).
            if !synced.active_construct_handles.is_empty() {
                synced.active_construct_handles.clear();
                synced.applied_pbr_key = None;
            }

            if synced.applied_pbr_key != Some(state_sig) {
                let custom_norm = load_optional_mod_image(
                    asset_server.as_deref(),
                    asset_root.as_deref(),
                    loaded,
                    visual_def.and_then(|v| v.normal_map_path.as_deref()),
                );
                let custom_orm = load_optional_mod_image(
                    asset_server.as_deref(),
                    asset_root.as_deref(),
                    loaded,
                    visual_def.and_then(|v| v.orm_map_path.as_deref()),
                );

                for (idx, &(mesh_ent, _)) in synced.mesh_entities.iter().enumerate() {
                    let orig_h = synced.orig_std_materials.get(idx).and_then(|o| o.as_ref());
                    let key = UnitPbrMaterialCacheKey {
                        base_mat_id: orig_h.map(|h| h.id()),
                        visual_spec_hash: v_hash,
                        quantized_rgba: q_rgba,
                        slot_is_neutral: slot == 0,
                        tier,
                        shield,
                    };
                    let pbr_handle = if let Some(existing) = cache.entries.get(&key) {
                        existing.clone()
                    } else {
                        let base_ref = orig_h.and_then(|h| mats.get(h)).unwrap_or(&default_std);
                        let mut mat_val = build_unit_pbr_team_material(
                            base_ref,
                            visual_def,
                            team_rgba,
                            slot == 0,
                            tier,
                            shield,
                            custom_norm.clone(),
                            custom_orm.clone(),
                            fow_tex_handle.clone(),
                            world_size,
                            fow_strength,
                            strategic_fade,
                            elapsed_secs,
                        );
                        mat_val.extension.micro_detail_tex = micro_tex_handle.clone();
                        let created = ext_mats.add(mat_val);
                        cache.entries.insert(key, created.clone());
                        created
                    };
                    commands
                        .entity(mesh_ent)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(pbr_handle));
                }
                synced.applied_pbr_key = Some(state_sig);
                synced.applied_material_key = Some((slot, tier));
            }
        }

        // Update live World-XZ FoW, micro-detail texture, orbital crossfade, and ember/shield animation uniforms across shared cache
        for mat_h in cache.entries.values() {
            if let Some(mat) = ext_mats.get_mut(mat_h) {
                mat.extension.params.damage_and_fx.w = strategic_fade;
                mat.extension.params.map_fow_params =
                    Vec4::new(world_size.x, world_size.y, fow_strength, elapsed_secs);
                if fow_tex_handle != Handle::default() && mat.extension.fow_tex != fow_tex_handle {
                    mat.extension.fow_tex = fow_tex_handle.clone();
                }
                if micro_tex_handle != Handle::default()
                    && mat.extension.micro_detail_tex != micro_tex_handle
                {
                    mat.extension.micro_detail_tex = micro_tex_handle.clone();
                }
            }
        }

        // Pass 3: Studio `MaterialPreviewOverrideResource` live PBR & Team-Mask preview on `PreviewStageEntity` (`Task 245`)
        if let Some(ovr) = preview_ovr.as_deref() {
            if ovr.enabled {
                let slot = ovr.preview_team_slot;
                let preview_rgba = resolve_accessible_team_color_in_mod(
                    slot,
                    slot == 1,
                    false,
                    cb_mode,
                    Some(loaded),
                );
                let ovr_hash = {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    hash_visual_pbr_spec(ovr.visual_override.as_ref()).hash(&mut h);
                    slot.hash(&mut h);
                    cb_mode.hash(&mut h);
                    h.finish()
                };
                for (ent, _m3d, std_opt, tracked_opt, name_opt) in &mut preview_mesh_q {
                    let is_floor_or_gizmo = name_opt.is_none()
                        && std_opt
                            .and_then(|m| mats.get(&m.0))
                            .is_some_and(|sm| sm.unlit || sm.metallic < 0.12);
                    if is_floor_or_gizmo {
                        continue;
                    }
                    let orig_h = match tracked_opt {
                        Some(ref t) => t.orig_std.clone(),
                        None => std_opt.map(|m| m.0.clone()),
                    };
                    let needs_update = tracked_opt
                        .as_ref()
                        .map(|t| t.applied_hash != ovr_hash)
                        .unwrap_or(true);
                    if needs_update {
                        let base_ref = orig_h
                            .as_ref()
                            .and_then(|h| mats.get(h))
                            .unwrap_or(&default_std);
                        let mut pbr_mat = build_unit_pbr_team_material(
                            base_ref,
                            ovr.visual_override.as_ref(),
                            preview_rgba,
                            slot == 0,
                            DamageVisualTier::Intact,
                            ShieldFresnelKind::None,
                            None,
                            None,
                            fow_tex_handle.clone(),
                            world_size,
                            0.0,
                            1.0,
                            elapsed_secs,
                        );
                        pbr_mat.extension.micro_detail_tex = micro_tex_handle.clone();
                        let handle = if let Some(existing) =
                            cache.preview_entries.get(&orig_h.as_ref().map(|h| h.id()))
                        {
                            if let Some(m) = ext_mats.get_mut(existing) {
                                *m = pbr_mat;
                            }
                            existing.clone()
                        } else {
                            let h_new = ext_mats.add(pbr_mat);
                            cache
                                .preview_entries
                                .insert(orig_h.as_ref().map(|h| h.id()), h_new.clone());
                            h_new
                        };
                        commands
                            .entity(ent)
                            .remove::<MeshMaterial3d<StandardMaterial>>()
                            .insert((
                                PreviewMeshOriginalMaterial {
                                    orig_std: orig_h,
                                    applied_hash: ovr_hash,
                                },
                                MeshMaterial3d(handle),
                            ));
                    }
                }
            }
        }
        return;
    }

    // Headless fallback when `Assets<UnitPbrTeamMaterial>` is not registered in a minimal test App
    for (_ent, vis, synced_opt) in &mut root_q {
        let Some(mut synced) = synced_opt else {
            continue;
        };
        if !synced.lod_configured || synced.mesh_entities.is_empty() {
            continue;
        };
        let Some(sim_ent) = world.entities.get(&vis.sim_id) else {
            continue;
        };
        let slot = resolve_player_team_color_slot(world, loaded, sim_ent.player_id);
        let hp_ratio = sim_ent.health / sim_ent.max_health.max(1.0);
        let tier = DamageVisualTier::from_hp_ratio(hp_ratio);
        let desired_key = (slot, tier);

        if synced.applied_material_key != Some(desired_key) {
            let shared_mat = palette.get_or_create_in_mod(&mut mats, slot, tier, Some(loaded));
            for &(mesh_ent, _) in &synced.mesh_entities {
                commands
                    .entity(mesh_ent)
                    .insert(MeshMaterial3d(shared_mat.clone()));
            }
            synced.applied_material_key = Some(desired_key);
        }
    }
}

#[cfg(test)]
mod tests;
