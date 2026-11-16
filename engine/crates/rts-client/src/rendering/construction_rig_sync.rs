use super::preview_stage::{PreviewStageCamera, PreviewStageEntity};
use super::world_overlays::should_use_3d_gltf_construction_rig;
use super::{
    build_default_fow_overlay_image, build_procedural_micro_detail_image,
    build_unit_pbr_team_material, compute_rig_vertical_profile, footprint_rig_half_extents,
    is_lod1_node_name, lod0_visibility_range, lod1_visibility_range,
    resolve_accessible_team_color_in_mod, resolve_mod_asset_path, resolve_player_team_color_slot,
    BuildingConstructionRig, CachedFogVisibility, ColorblindPaletteMode,
    ColorblindSettingsResource, ConstructionRigCache, ConstructionRigFaction, DamageVisualTier,
    DiscoveredRigNodes, FogOfWarOverlayImage, FogOfWarPlaneEntity, MapEnvironmentEntity,
    MicroDetailTextureResource, PlacementGhostModel, Ra2EntityVisualMode, ShieldFresnelKind,
    SimVisualEntity, SmoothFogTemporalState, UnitPbrTeamMaterial,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::vfx::{GpuParticleBurstBuffer, GpuParticleInstance, GpuParticlePoolResources};
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{TeamMaskMode, VisualDef};
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::HashMap;

const REF_HALF_EXTENT: f32 = 1.80;
const TOWER_COLLAR_INSET: f32 = 0.22;
const REF_SPAN_HALF: f32 = REF_HALF_EXTENT - TOWER_COLLAR_INSET;

type RigNodeFilter = (
    Without<SimVisualEntity>,
    Without<BuildingConstructionRig>,
    Without<FogOfWarPlaneEntity>,
    Without<PlacementGhostModel>,
    Without<PreviewStageEntity>,
);

type RigEntityFilter = (
    Without<SimVisualEntity>,
    Without<FogOfWarPlaneEntity>,
    Without<PlacementGhostModel>,
    Without<PreviewStageEntity>,
);

fn resolve_owner_for_building(
    world: &SimWorld,
    loaded: &LoadedMod,
    vis: &SimVisualEntity,
    cache: &ConstructionRigCache,
) -> (u8, u8, ConstructionRigFaction) {
    if let Some(sim_ent) = world.entities.get(&vis.sim_id) {
        let pid = sim_ent.player_id;
        let slot = resolve_player_team_color_slot(world, loaded, pid);
        let p_fac = world.players.get(&pid).map(|p| p.faction.as_str());
        let b_fac = loaded
            .buildings
            .get(&vis.entity_type_id)
            .map(|b| b.faction.as_str())
            .filter(|s| !s.is_empty());
        return (
            pid,
            slot,
            ConstructionRigFaction::from_faction_or_entity_id(b_fac.or(p_fac), &vis.entity_type_id),
        );
    }
    if let Some(&cached) = cache.known_building_owners.get(&vis.sim_id) {
        return cached;
    }
    let b_fac = loaded
        .buildings
        .get(&vis.entity_type_id)
        .map(|b| b.faction.as_str())
        .filter(|s| !s.is_empty());
    (
        1,
        1,
        ConstructionRigFaction::from_faction_or_entity_id(b_fac, &vis.entity_type_id),
    )
}

#[allow(clippy::type_complexity)]
fn discover_rig_hierarchy(
    root: Entity,
    children_q: &Query<&Children>,
    node_q: &Query<
        (
            Option<&Name>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        RigNodeFilter,
    >,
) -> Option<(
    DiscoveredRigNodes,
    Vec<(Entity, bool)>,
    Vec<Option<Handle<StandardMaterial>>>,
)> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let mut nodes = DiscoveredRigNodes::default();
    let mut meshes = Vec::new();
    let mut orig_mats = Vec::new();
    let mut stack = vec![(root, false)];

    while let Some((curr, parent_is_lod1)) = stack.pop() {
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                let mut is_lod1 = parent_is_lod1;
                if let Ok((name_opt, mesh_opt, mat_opt)) = node_q.get(child) {
                    if let Some(name) = name_opt {
                        let s = name.as_str();
                        if is_lod1_node_name(s) {
                            is_lod1 = true;
                        } else {
                            match s {
                                "CornerTower_NW" => nodes.corner_nw = Some(child),
                                "CornerTower_NE" => nodes.corner_ne = Some(child),
                                "CornerTower_SE" => nodes.corner_se = Some(child),
                                "CornerTower_SW" => nodes.corner_sw = Some(child),
                                "SpanBeam_N" => nodes.span_n = Some(child),
                                "SpanBeam_S" => nodes.span_s = Some(child),
                                "SpanBeam_E" => nodes.span_e = Some(child),
                                "SpanBeam_W" => nodes.span_w = Some(child),
                                "CraneMast" => nodes.crane_mast = Some(child),
                                "CraneJib" => nodes.crane_jib = Some(child),
                                "SparkMuzzle_01" => nodes.spark_muzzle = Some(child),
                                _ => {}
                            }
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
    if meshes.is_empty() || nodes.crane_mast.is_none() {
        None
    } else {
        Some((nodes, meshes, orig_mats))
    }
}

fn emit_welding_spark_and_dust_burst(
    burst: &mut GpuParticleBurstBuffer,
    weld_pos: Vec3,
    base_pos: Vec3,
    hw: f32,
    hd: f32,
    rig_faction: ConstructionRigFaction,
    seed_base: u64,
) {
    let hash01 = |s: u64| -> f32 {
        let mut x = s.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (((x ^ (x >> 31)) & 0x00FF_FFFF) as f32) / (0x0100_0000 as f32)
    };
    let (primary_col, secondary_col) = match rig_faction {
        ConstructionRigFaction::Allies => ([0.52, 0.96, 1.00, 1.0], [1.00, 0.96, 0.82, 1.0]),
        ConstructionRigFaction::Soviets => ([1.00, 0.76, 0.20, 1.0], [1.00, 0.44, 0.10, 1.0]),
        ConstructionRigFaction::Yuri => ([0.24, 1.00, 0.56, 1.0], [0.88, 0.46, 1.00, 1.0]),
    };

    for i in 0..8u64 {
        let s = seed_base.wrapping_add(i * 37);
        let ang = hash01(s) * std::f32::consts::TAU;
        let spd = 1.35 + hash01(s + 1) * 2.45;
        burst.emit_with_drag(
            GpuParticleInstance {
                position: weld_pos
                    + Vec3::new(
                        (hash01(s + 3) - 0.5) * 0.18,
                        0.0,
                        (hash01(s + 4) - 0.5) * 0.18,
                    ),
                velocity: Vec3::new(
                    ang.cos() * spd,
                    0.65 + hash01(s + 2) * 2.10,
                    ang.sin() * spd,
                ),
                age: 0.0,
                lifetime: 0.34 + hash01(s + 5) * 0.28,
                gravity: 9.8,
                start_size: 0.13 + hash01(s + 6) * 0.06,
                end_size: 0.03,
                color_rgba: if i % 3 == 0 {
                    secondary_col
                } else {
                    primary_col
                },
            },
            0.35,
        );
    }

    let ds = seed_base.wrapping_add(901);
    let sx = if hash01(ds) > 0.5 { 1.0 } else { -1.0 };
    let sz = if hash01(ds + 1) > 0.5 { 1.0 } else { -1.0 };
    burst.emit_with_drag(
        GpuParticleInstance {
            position: Vec3::new(
                base_pos.x + sx * hw * 0.92,
                base_pos.y + 0.16,
                base_pos.z + sz * hd * 0.92,
            ),
            velocity: Vec3::new(sx * 0.45, 0.55 + hash01(ds + 2) * 0.45, sz * 0.45),
            age: 0.0,
            lifetime: 0.62,
            gravity: -0.25,
            start_size: 0.26,
            end_size: 0.58,
            color_rgba: [0.64, 0.62, 0.58, 0.38],
        },
        0.85,
    );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_construction_rigs(
    mut commands: Commands,
    (time, asset_server, asset_root, mod_res, sim_res, visual_mode): (
        Res<Time>,
        Option<Res<AssetServer>>,
        Option<Res<AssetRootResource>>,
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
        Option<Res<Ra2EntityVisualMode>>,
    ),
    (interaction, cb_settings, cached_fow, smooth_fow, fow_img_res, micro_detail_res): (
        Option<Res<ClientInteractionState>>,
        Option<Res<ColorblindSettingsResource>>,
        Option<Res<CachedFogVisibility>>,
        Option<Res<SmoothFogTemporalState>>,
        Option<Res<FogOfWarOverlayImage>>,
        Option<Res<MicroDetailTextureResource>>,
    ),
    (mut rig_cache, mut gpu_pool, materials, mut unit_pbr_mats, mut images): (
        ResMut<ConstructionRigCache>,
        Option<ResMut<GpuParticlePoolResources>>,
        Option<ResMut<Assets<StandardMaterial>>>,
        Option<ResMut<Assets<UnitPbrTeamMaterial>>>,
        Option<ResMut<Assets<Image>>>,
    ),
    cam_rig_q: Query<
        &RtsCameraRig,
        (
            Without<PreviewStageCamera>,
            Without<SimVisualEntity>,
            Without<BuildingConstructionRig>,
            Without<PreviewStageEntity>,
        ),
    >,
    buildings_q: Query<
        (&SimVisualEntity, &Transform, &Visibility),
        (
            Without<BuildingConstructionRig>,
            Without<FogOfWarPlaneEntity>,
            Without<PlacementGhostModel>,
            Without<PreviewStageEntity>,
        ),
    >,
    mut rigs_q: Query<
        (
            Entity,
            &mut BuildingConstructionRig,
            &mut Transform,
            &mut Visibility,
        ),
        RigEntityFilter,
    >,
    children_q: Query<&Children>,
    node_inspect_q: Query<
        (
            Option<&Name>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        RigNodeFilter,
    >,
    mut child_tf_q: Query<&mut Transform, RigNodeFilter>,
    global_tf_q: Query<&GlobalTransform>,
) {
    let (Some(mod_res), Some(sim_res)) = (mod_res, sim_res) else {
        return;
    };
    let (world, loaded, dt, elapsed_secs) = (
        &sim_res.0,
        &mod_res.0,
        time.delta_secs(),
        time.elapsed_secs(),
    );

    for ent in world.entities.values() {
        if let EntityKind::Building { building_id } = &ent.kind {
            let slot = resolve_player_team_color_slot(world, loaded, ent.player_id);
            let p_fac = world
                .players
                .get(&ent.player_id)
                .map(|p| p.faction.as_str());
            let b_fac = loaded
                .buildings
                .get(building_id)
                .map(|b| b.faction.as_str())
                .filter(|s| !s.is_empty());
            let rig_fac =
                ConstructionRigFaction::from_faction_or_entity_id(b_fac.or(p_fac), building_id);
            rig_cache
                .known_building_owners
                .insert(ent.id, (ent.player_id, slot, rig_fac));
        }
    }

    let active_mode = visual_mode.as_deref().copied().unwrap_or_default();
    if !should_use_3d_gltf_construction_rig(loaded, active_mode) {
        for (rig_ent, _, _, _) in &rigs_q {
            commands.entity(rig_ent).despawn_recursive();
        }
        return;
    }

    let Some(srv) = asset_server.as_deref() else {
        return;
    };
    if rig_cache.loaded_mod_root.as_deref() != Some(loaded.root_dir.as_path()) {
        rig_cache.scene_handles.clear();
        rig_cache.known_building_owners.clear();
        for fac in [
            ConstructionRigFaction::Allies,
            ConstructionRigFaction::Soviets,
            ConstructionRigFaction::Yuri,
        ] {
            let scene_path = if let Some(ar) = asset_root.as_deref() {
                resolve_mod_asset_path(&ar.0, &loaded.root_dir, fac.model_rel_path())
            } else {
                format!("{}#Scene0", fac.model_rel_path())
            };
            rig_cache.scene_handles.insert(fac, srv.load(scene_path));
        }
        rig_cache.loaded_mod_root = Some(loaded.root_dir.clone());
    }

    type TargetRigSpec = (
        [u32; 2],
        Vec3,
        f32,
        Visibility,
        f32,
        u8,
        u8,
        ConstructionRigFaction,
    );
    let mut desired: HashMap<u64, TargetRigSpec> = HashMap::new();
    for (vis, bldg_tf, bldg_vis) in &buildings_q {
        let Some(fp) = vis.building_footprint else {
            continue;
        };
        let is_deconstructing = vis.deconstruct_progress.is_some();
        if !is_deconstructing && vis.spawn_progress >= 0.996 {
            continue;
        }
        let prog = vis
            .deconstruct_progress
            .unwrap_or(vis.spawn_progress)
            .clamp(0.0, 1.0);
        if is_deconstructing && prog <= 0.004 {
            continue;
        }
        let (_, yaw, _) = bldg_tf.rotation.to_euler(EulerRot::YXZ);
        let (pid, slot, rig_fac) = resolve_owner_for_building(world, loaded, vis, &rig_cache);
        desired.insert(
            vis.sim_id,
            (
                fp,
                bldg_tf.translation,
                yaw,
                *bldg_vis,
                prog,
                pid,
                slot,
                rig_fac,
            ),
        );
    }

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

    let rig_visual_def = VisualDef {
        team_color_tint: true,
        team_mask_mode: TeamMaskMode::AlphaMask,
        emissive_boost: Some(0.32),
        ..Default::default()
    };
    let default_std = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.48,
        metallic: 0.45,
        ..default()
    };

    for (rig_ent, mut rig, mut rig_tf, mut rig_vis) in &mut rigs_q {
        let Some((fp, bldg_pos, bldg_yaw, root_vis, prog, pid, slot, rig_fac)) =
            desired.remove(&rig.sim_id)
        else {
            commands.entity(rig_ent).despawn_recursive();
            continue;
        };
        if rig.rig_faction != rig_fac {
            commands.entity(rig_ent).despawn_recursive();
            desired.insert(
                rig.sim_id,
                (fp, bldg_pos, bldg_yaw, root_vis, prog, pid, slot, rig_fac),
            );
            continue;
        }

        rig.cached_player_id = pid;
        rig.cached_color_slot = slot;
        rig_tf.translation = bldg_pos;
        rig_tf.rotation = Quat::from_rotation_y(bldg_yaw);
        rig_tf.scale = Vec3::ONE;
        if *rig_vis != root_vis {
            *rig_vis = root_vis;
        }

        if !rig.nodes_discovered {
            if let Some((nodes, meshes, orig_mats)) =
                discover_rig_hierarchy(rig_ent, &children_q, &node_inspect_q)
            {
                for &(m_ent, is_lod1) in &meshes {
                    commands.entity(m_ent).insert(if is_lod1 {
                        lod1_visibility_range()
                    } else {
                        lod0_visibility_range()
                    });
                }
                rig.nodes = nodes;
                rig.mesh_entities = meshes;
                rig.orig_std_materials = orig_mats;
                rig.nodes_discovered = true;
            }
        }
        if !rig.nodes_discovered {
            continue;
        }

        let (hw, hd) = footprint_rig_half_extents(fp);
        let (y_scaf, y_crane, retract_damp) = compute_rig_vertical_profile(prog);

        if let (Some(std_mats), Some(ext_mats)) =
            (materials.as_deref(), unit_pbr_mats.as_deref_mut())
        {
            let is_ally = pid != 0 && !world.are_hostile(local_pid, pid);
            let team_rgba = resolve_accessible_team_color_in_mod(
                slot,
                pid == local_pid,
                is_ally,
                cb_mode,
                Some(loaded),
            );
            let q_rgb = [
                (team_rgba[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (team_rgba[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (team_rgba[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            ];
            let color_key = (q_rgb, slot == 0);
            let rig_c_params = Vec4::new(0.998, bldg_pos.y, -100.0, 0.0);

            if rig.applied_color_key != Some(color_key)
                || rig.material_handles.len() != rig.mesh_entities.len()
            {
                let mesh_list = rig.mesh_entities.clone();
                let orig_list = rig.orig_std_materials.clone();
                rig.material_handles.clear();
                for (idx, (m_ent, _)) in mesh_list.into_iter().enumerate() {
                    let base_ref = orig_list
                        .get(idx)
                        .and_then(|o| o.as_ref())
                        .and_then(|h| std_mats.get(h))
                        .unwrap_or(&default_std);
                    let mut mat_val = build_unit_pbr_team_material(
                        base_ref,
                        Some(&rig_visual_def),
                        team_rgba,
                        slot == 0,
                        DamageVisualTier::Intact,
                        ShieldFresnelKind::None,
                        None,
                        None,
                        fow_tex_handle.clone(),
                        world_size,
                        fow_strength,
                        strategic_fade,
                        elapsed_secs,
                    );
                    mat_val.extension.micro_detail_tex = micro_tex_handle.clone();
                    mat_val.extension.params.construction_params = rig_c_params;
                    let h_mat = ext_mats.add(mat_val);
                    rig.material_handles.push(h_mat.clone());
                    commands
                        .entity(m_ent)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(h_mat));
                }
                rig.applied_color_key = Some(color_key);
            } else {
                for h_mat in &rig.material_handles {
                    if let Some(mat) = ext_mats.get_mut(h_mat) {
                        mat.extension.params.construction_params = rig_c_params;
                        mat.extension.params.damage_and_fx.w = strategic_fade;
                        mat.extension.params.map_fow_params =
                            Vec4::new(world_size.x, world_size.y, fow_strength, elapsed_secs);
                    }
                }
            }
        }

        for (opt_e, pos) in [
            (rig.nodes.corner_nw, Vec3::new(-hw, y_scaf, -hd)),
            (rig.nodes.corner_ne, Vec3::new(hw, y_scaf, -hd)),
            (rig.nodes.corner_se, Vec3::new(hw, y_scaf, hd)),
            (rig.nodes.corner_sw, Vec3::new(-hw, y_scaf, hd)),
        ] {
            if let Some(Ok(mut tf)) = opt_e.map(|e| child_tf_q.get_mut(e)) {
                tf.translation = pos;
                tf.scale = Vec3::ONE;
            }
        }

        let sx = ((hw - TOWER_COLLAR_INSET).max(0.35) / REF_SPAN_HALF).clamp(0.25, 3.0);
        let sz = ((hd - TOWER_COLLAR_INSET).max(0.35) / REF_SPAN_HALF).clamp(0.25, 3.0);
        for (opt_e, pos, sc) in [
            (
                rig.nodes.span_n,
                Vec3::new(0.0, y_scaf, -hd),
                Vec3::new(sx, 1.0, 1.0),
            ),
            (
                rig.nodes.span_s,
                Vec3::new(0.0, y_scaf, hd),
                Vec3::new(sx, 1.0, 1.0),
            ),
            (
                rig.nodes.span_e,
                Vec3::new(hw, y_scaf, 0.0),
                Vec3::new(1.0, 1.0, sz),
            ),
            (
                rig.nodes.span_w,
                Vec3::new(-hw, y_scaf, 0.0),
                Vec3::new(1.0, 1.0, sz),
            ),
        ] {
            if let Some(Ok(mut tf)) = opt_e.map(|e| child_tf_q.get_mut(e)) {
                tf.translation = pos;
                tf.scale = sc;
            }
        }

        let (mast_x, mast_z) = (hw + 0.38, -hd - 0.38);
        if let Some(Ok(mut tf)) = rig.nodes.crane_mast.map(|e| child_tf_q.get_mut(e)) {
            tf.translation = Vec3::new(mast_x, y_crane, mast_z);
            tf.scale = Vec3::ONE;
        }
        if let Some(Ok(mut tf)) = rig.nodes.crane_jib.map(|e| child_tf_q.get_mut(e)) {
            let delta_aim = (-mast_x).atan2(-mast_z) - (-std::f32::consts::FRAC_PI_4);
            let phase = bldg_pos.x * 0.37 + bldg_pos.z * 0.23 + (rig.sim_id as f32) * 0.71;
            let sweep = (elapsed_secs * 0.95 + phase).sin() * 0.46 * retract_damp;
            tf.translation = Vec3::new(0.0, 5.40, 0.0);
            tf.rotation = Quat::from_rotation_y(delta_aim + sweep)
                * Quat::from_rotation_x(-0.26 * (1.0 - retract_damp));
            tf.scale = Vec3::ONE;
        }

        if root_vis != Visibility::Hidden && prog > 0.08 && prog < 0.88 {
            rig.spark_timer += dt;
            if rig.spark_timer >= 0.11 {
                rig.spark_timer = 0.0;
                if let Some(pool) = gpu_pool.as_deref_mut() {
                    let weld_pos = rig
                        .nodes
                        .spark_muzzle
                        .and_then(|se| global_tf_q.get(se).ok())
                        .map(|gt| gt.translation())
                        .filter(|p| p.y > bldg_pos.y + 0.15)
                        .unwrap_or_else(|| bldg_pos + Vec3::new(0.0, (2.8 * prog).max(0.55), 0.0));
                    let seed = rig
                        .sim_id
                        .wrapping_mul(1_000_003)
                        .wrapping_add((elapsed_secs * 60.0) as u64);
                    emit_welding_spark_and_dust_burst(
                        &mut pool.burst_buffer,
                        weld_pos,
                        bldg_pos,
                        hw,
                        hd,
                        rig.rig_faction,
                        seed,
                    );
                }
            }
        }
    }

    for (sim_id, (_fp, bldg_pos, bldg_yaw, root_vis, _prog, pid, slot, rig_fac)) in desired {
        let Some(scene_h) = rig_cache.scene_handles.get(&rig_fac).cloned() else {
            continue;
        };
        commands.spawn((
            MapEnvironmentEntity,
            BuildingConstructionRig {
                sim_id,
                rig_faction: rig_fac,
                cached_player_id: pid,
                cached_color_slot: slot,
                nodes_discovered: false,
                nodes: DiscoveredRigNodes::default(),
                mesh_entities: Vec::new(),
                orig_std_materials: Vec::new(),
                material_handles: Vec::new(),
                applied_color_key: None,
                spark_timer: 0.08,
            },
            SceneRoot(scene_h),
            root_vis,
            Transform::from_translation(bldg_pos).with_rotation(Quat::from_rotation_y(bldg_yaw)),
        ));
    }
}
