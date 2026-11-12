use super::data_driven_vfx_emitters::step_vfx_instance;
pub use super::data_driven_vfx_emitters::{
    resolve_vfx_effect_def, synthesize_fallback_vfx_effect, ActiveDataDrivenVfxInstance,
    ActiveEmitterRuntime, DataDrivenVfxBeamBatchEntity, DataDrivenVfxRuntimeState,
    PreviewStageVfxBatchEntity, PreviewStageVfxParticleEntity, MAX_ACTIVE_DATA_DRIVEN_VFX,
};
use super::{
    new_empty_particle_billboard_mesh, new_empty_vfx_batch_mesh, DecalSystemState,
    GpuParticlePoolResources, VfxBeamAndRingBatchBuilder, VfxPreviewStageState,
};
use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{
    CachedFogVisibility, MapEnvironmentEntity, PreviewStageCamera, PreviewStageEntity,
    SocketRegistry, PREVIEW_STAGE_LAYER,
};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{ProjectileStyle, VfxEffectDef};
use rts_simulation::world::{EntityKind, SimWorld};
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
fn trigger_sim_data_driven_vfx(
    runtime: &mut DataDrivenVfxRuntimeState,
    loaded: &LoadedMod,
    world: &SimWorld,
    cached_fow: &CachedFogVisibility,
    pov_pid: u8,
    mut socket_reg: Option<&mut SocketRegistry>,
    mut decal_state: Option<&mut DecalSystemState>,
    mut camera_rig: Option<&mut RtsCameraRig>,
) {
    if world.tick < runtime.last_sim_tick {
        runtime.reset_battlefield();
    }

    let mut alive_ambient = BTreeSet::new();
    for ent in world.entities.values() {
        if ent.health <= 0.0 {
            continue;
        }
        let (amb_opt, death_opt, abilities) = match &ent.kind {
            EntityKind::Unit { unit_id } => loaded
                .units
                .get(unit_id)
                .map(|u| {
                    (
                        u.ambient_vfx_id.as_deref(),
                        u.death_vfx_id.as_deref(),
                        u.abilities.as_slice(),
                    )
                })
                .unwrap_or((None, None, &[])),
            EntityKind::Building { building_id } => loaded
                .buildings
                .get(building_id)
                .map(|b| {
                    (
                        b.ambient_vfx_id.as_deref(),
                        b.death_vfx_id.as_deref(),
                        &[][..],
                    )
                })
                .unwrap_or((None, None, &[])),
        };
        if let Some(d_id) = death_opt.filter(|s| !s.is_empty()) {
            runtime
                .known_entity_death_vfx
                .insert(ent.id, d_id.to_string());
        }

        let is_online = ent.construction_progress >= 0.99 && !ent.power_disabled && !ent.burrowed;
        if let Some(amb_id) = amb_opt.filter(|s| !s.is_empty() && is_online) {
            if cached_fow.is_entity_visible(world, loaded, ent) {
                alive_ambient.insert(ent.id);
                if !runtime.active_ambient_entities.contains(&ent.id) {
                    if let Some(mut eff) = resolve_vfx_effect_def(loaded, amb_id).cloned() {
                        eff.looping = true;
                        let pos =
                            Vec3::new(ent.position[0], ent.position[1] + 1.1, ent.position[2]);
                        runtime.spawn_effect(
                            &eff,
                            pos,
                            pos + Vec3::Y * 1.5,
                            Some(ent.id),
                            None,
                            None,
                            None,
                        );
                        runtime.active_ambient_entities.insert(ent.id);
                    }
                }
            }
        }

        if !abilities.is_empty() && !ent.ability_cooldowns.is_empty() {
            let prev_cds = runtime
                .known_ability_cooldowns
                .entry(ent.id)
                .or_insert_with(|| ent.ability_cooldowns.clone())
                .clone();
            for (slot_idx, &curr_cd) in ent.ability_cooldowns.iter().enumerate() {
                let prev_cd = prev_cds.get(slot_idx).copied().unwrap_or(0.0);
                if curr_cd > prev_cd + 0.08 {
                    if let Some(ab_def) = abilities.get(slot_idx) {
                        let caster_pos =
                            Vec3::new(ent.position[0], ent.position[1] + 0.85, ent.position[2]);
                        let tgt_pos = ent
                            .target_entity_id
                            .and_then(|tid| world.entities.get(&tid))
                            .map(|te| {
                                Vec3::new(te.position[0], te.position[1] + 0.85, te.position[2])
                            })
                            .or_else(|| {
                                ent.attack_ground_pos
                                    .map(|p| Vec3::new(p[0], p[1] + 0.35, p[2]))
                            })
                            .unwrap_or(caster_pos);
                        if cached_fow.is_entity_visible(world, loaded, ent) {
                            if let Some(cid) = ab_def.cast_vfx_id.as_deref() {
                                if let Some(eff) = resolve_vfx_effect_def(loaded, cid) {
                                    runtime.spawn_effect(
                                        eff,
                                        caster_pos,
                                        tgt_pos,
                                        None,
                                        None,
                                        decal_state.as_deref_mut(),
                                        camera_rig.as_deref_mut(),
                                    );
                                }
                            }
                        }
                        if cached_fow.is_world_pos_visible(world, tgt_pos.to_array()) {
                            if let Some(iid) = ab_def.impact_vfx_id.as_deref() {
                                if let Some(eff) = resolve_vfx_effect_def(loaded, iid) {
                                    runtime.spawn_effect(
                                        eff,
                                        tgt_pos,
                                        caster_pos,
                                        None,
                                        None,
                                        decal_state.as_deref_mut(),
                                        camera_rig.as_deref_mut(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            runtime
                .known_ability_cooldowns
                .insert(ent.id, ent.ability_cooldowns.clone());
        }
    }

    let mut alive_proj = BTreeSet::new();
    for proj in &world.projectiles {
        let Some(wdef) = loaded.weapons.get(&proj.weapon_id) else {
            continue;
        };
        let Some(pvfx_id) = wdef.projectile_vfx_id.as_deref().filter(|s| !s.is_empty()) else {
            continue;
        };
        let is_ally =
            proj.attacker_player == pov_pid || !world.are_hostile(pov_pid, proj.attacker_player);
        if !is_ally && !cached_fow.is_world_pos_visible(world, proj.current_pos) {
            continue;
        }
        alive_proj.insert(proj.id);
        if !runtime.active_projectile_vfx.contains(&proj.id) {
            if let Some(mut eff) = resolve_vfx_effect_def(loaded, pvfx_id).cloned() {
                eff.looping = true;
                let p = Vec3::from_array(proj.current_pos);
                let tgt = Vec3::from_array(proj.target_pos);
                runtime.spawn_effect(&eff, p, tgt, None, Some(proj.id), None, None);
                runtime.active_projectile_vfx.insert(proj.id);
            }
        }
    }

    runtime.active_ambient_entities = alive_ambient.clone();
    runtime.active_projectile_vfx = alive_proj.clone();
    runtime.active_instances.retain_mut(|inst| {
        if let Some(eid) = inst.attached_entity_id {
            if !alive_ambient.contains(&eid) {
                return false;
            }
            if let Some(ent) = world.entities.get(&eid) {
                inst.origin = Vec3::new(ent.position[0], ent.position[1] + 1.1, ent.position[2]);
            }
        }
        if let Some(pid) = inst.attached_projectile_id {
            if !alive_proj.contains(&pid) {
                return false;
            }
            if let Some(p) = world.projectiles.iter().find(|pr| pr.id == pid) {
                inst.origin = Vec3::from_array(p.current_pos);
                let travel = Vec3::from_array(p.target_pos) - Vec3::from_array(p.start_pos);
                if travel.length_squared() > 1e-4 {
                    inst.forward = -travel.normalize();
                }
            }
        }
        true
    });

    if world.tick == runtime.last_sim_tick {
        return;
    }
    runtime.last_sim_tick = world.tick;

    for ev in &world.combat_events {
        let Some(wdef) = loaded.weapons.get(&ev.weapon_id) else {
            continue;
        };
        if wdef.muzzle_vfx_id.is_none() && wdef.impact_vfx_id.is_none() {
            continue;
        }
        let att_ally = world.entities.get(&ev.attacker_id).is_some_and(|e| {
            e.player_id == pov_pid || (e.player_id != 0 && !world.are_hostile(pov_pid, e.player_id))
        });
        let tgt_ally = world.entities.get(&ev.target_id).is_some_and(|e| {
            e.player_id == pov_pid || (e.player_id != 0 && !world.are_hostile(pov_pid, e.player_id))
        });
        let muzzle_pos = socket_reg
            .as_deref_mut()
            .and_then(|reg| reg.next_muzzle_pos(ev.attacker_id))
            .unwrap_or_else(|| {
                Vec3::new(
                    ev.attacker_pos[0],
                    ev.attacker_pos[1] + 1.1,
                    ev.attacker_pos[2],
                )
            });
        let impact_pos = Vec3::new(ev.target_pos[0], ev.target_pos[1] + 0.85, ev.target_pos[2]);

        if att_ally || cached_fow.is_world_pos_visible(world, ev.attacker_pos) {
            if let Some(mid) = wdef.muzzle_vfx_id.as_deref().filter(|s| !s.is_empty()) {
                let eff = resolve_vfx_effect_def(loaded, mid)
                    .cloned()
                    .unwrap_or_else(|| synthesize_fallback_vfx_effect(ev.projectile_style));
                runtime.spawn_effect(
                    &eff,
                    muzzle_pos,
                    impact_pos,
                    None,
                    None,
                    decal_state.as_deref_mut(),
                    camera_rig.as_deref_mut(),
                );
            }
        }
        if tgt_ally || cached_fow.is_world_pos_visible(world, ev.target_pos) {
            if let Some(iid) = wdef.impact_vfx_id.as_deref().filter(|s| !s.is_empty()) {
                let eff = resolve_vfx_effect_def(loaded, iid)
                    .cloned()
                    .unwrap_or_else(|| synthesize_fallback_vfx_effect(ev.projectile_style));
                runtime.spawn_effect(
                    &eff,
                    impact_pos,
                    muzzle_pos,
                    None,
                    None,
                    decal_state.as_deref_mut(),
                    camera_rig.as_deref_mut(),
                );
            }
        }
    }

    for dest in &world.destruction_events {
        let Some(death_id) = runtime.known_entity_death_vfx.remove(&dest.entity_id) else {
            continue;
        };
        if dest.is_sold {
            continue;
        }
        let non_hostile = dest.player_id == pov_pid
            || (dest.player_id != 0 && !world.are_hostile(pov_pid, dest.player_id));
        if !non_hostile && !cached_fow.is_world_pos_visible(world, dest.position) {
            continue;
        }
        if let Some(eff) = resolve_vfx_effect_def(loaded, &death_id) {
            let center = Vec3::new(
                dest.position[0],
                dest.position[1].max(0.0) + 0.85,
                dest.position[2],
            );
            runtime.spawn_effect(
                eff,
                center,
                center + Vec3::Y * 2.5,
                None,
                None,
                decal_state.as_deref_mut(),
                camera_rig.as_deref_mut(),
            );
        }
    }
}

fn resolve_preview_effect(
    preview: &VfxPreviewStageState,
    loaded: Option<&LoadedMod>,
) -> Option<VfxEffectDef> {
    if let Some(ref ovr) = preview.override_effect {
        return Some(ovr.clone());
    }
    let id = preview.active_effect_id.as_deref()?.trim();
    if id.is_empty() {
        return None;
    }
    if let Some(l) = loaded {
        if let Some(found) = resolve_vfx_effect_def(l, id) {
            return Some(found.clone());
        }
    }
    let low = id.to_ascii_lowercase();
    let style = if low.contains("tesla") {
        ProjectileStyle::Tesla
    } else if low.contains("laser") || low.contains("prism") {
        ProjectileStyle::Laser
    } else if low.contains("rad") {
        ProjectileStyle::Radiation
    } else if low.contains("sonic") {
        ProjectileStyle::Sonic
    } else if low.contains("psi") || low.contains("mind") {
        ProjectileStyle::MindControl
    } else if low.contains("missile") || low.contains("rocket") {
        ProjectileStyle::Missile
    } else {
        ProjectileStyle::Bullet
    };
    Some(synthesize_fallback_vfx_effect(style))
}

/// Advances battlefield data-driven `VfxEffectDef` emitters AND renders live Studio 3D Preview Stage
/// (`RenderLayers::layer(3)`) VFX previews driven by `VfxPreviewStageState` (`Task 244`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_data_driven_vfx_and_preview(
    mut commands: Commands,
    time: Res<Time>,
    (mod_res, sim_res, interaction, cached_fow): (
        Option<Res<ModResource>>,
        Option<Res<SimWorldResource>>,
        Option<Res<ClientInteractionState>>,
        Option<Res<CachedFogVisibility>>,
    ),
    (mut runtime, mut preview_state, mut gpu_pool, mut decal_state, mut socket_reg): (
        ResMut<DataDrivenVfxRuntimeState>,
        Option<ResMut<VfxPreviewStageState>>,
        Option<ResMut<GpuParticlePoolResources>>,
        Option<ResMut<DecalSystemState>>,
        Option<ResMut<SocketRegistry>>,
    ),
    (mut meshes, mut images, mut materials): (
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<Image>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
    ),
    (mut camera_q, preview_cam_q): (
        Query<&mut RtsCameraRig, Without<PreviewStageCamera>>,
        Query<&Transform, With<PreviewStageCamera>>,
    ),
    (bf_beam_q, prev_beam_q, prev_part_q): (
        Query<Entity, With<DataDrivenVfxBeamBatchEntity>>,
        Query<Entity, With<PreviewStageVfxBatchEntity>>,
        Query<Entity, With<PreviewStageVfxParticleEntity>>,
    ),
) {
    let dt = time.delta_secs();
    if let (Some(ref mut mesh_assets), Some(ref mut mat_assets)) =
        (meshes.as_deref_mut(), materials.as_deref_mut())
    {
        if !runtime.initialized {
            runtime.initialized = true;
            runtime.beam_batch_mesh = mesh_assets.add(new_empty_vfx_batch_mesh());
            runtime.preview_beam_mesh = mesh_assets.add(new_empty_vfx_batch_mesh());
            runtime.preview_particle_mesh = mesh_assets.add(new_empty_particle_billboard_mesh());
            runtime.beam_batch_material = mat_assets.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: LinearRgba::new(1.85, 1.85, 1.85, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });
            let disc_tex = images
                .as_deref_mut()
                .map(|imgs| imgs.add(super::build_soft_particle_disc_image()));
            runtime.preview_particle_material = mat_assets.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: disc_tex,
                emissive: LinearRgba::new(1.45, 1.35, 1.20, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });
        }
        if bf_beam_q.is_empty() {
            commands.spawn((
                MapEnvironmentEntity,
                DataDrivenVfxBeamBatchEntity,
                Mesh3d(runtime.beam_batch_mesh.clone()),
                MeshMaterial3d(runtime.beam_batch_material.clone()),
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                bevy::render::view::NoFrustumCulling,
                Transform::IDENTITY,
            ));
        }
        if prev_beam_q.is_empty() {
            commands.spawn((
                PreviewStageEntity,
                PreviewStageVfxBatchEntity,
                Mesh3d(runtime.preview_beam_mesh.clone()),
                MeshMaterial3d(runtime.beam_batch_material.clone()),
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                bevy::render::view::NoFrustumCulling,
                RenderLayers::layer(PREVIEW_STAGE_LAYER),
                Transform::IDENTITY,
            ));
        }
        if prev_part_q.is_empty() {
            commands.spawn((
                PreviewStageEntity,
                PreviewStageVfxParticleEntity,
                Mesh3d(runtime.preview_particle_mesh.clone()),
                MeshMaterial3d(runtime.preview_particle_material.clone()),
                bevy::pbr::NotShadowCaster,
                bevy::pbr::NotShadowReceiver,
                bevy::render::view::NoFrustumCulling,
                RenderLayers::layer(PREVIEW_STAGE_LAYER),
                Transform::IDENTITY,
            ));
        }
    }

    if let (Some(mres), Some(sres), Some(fow)) = (
        mod_res.as_deref(),
        sim_res.as_deref(),
        cached_fow.as_deref(),
    ) {
        let pov_pid = interaction
            .as_deref()
            .map(|i| CachedFogVisibility::effective_pov_and_fog(&sres.0, i).0)
            .unwrap_or(1);
        let mut rig_mut = camera_q.get_single_mut().ok();
        trigger_sim_data_driven_vfx(
            &mut runtime,
            &mres.0,
            &sres.0,
            fow,
            pov_pid,
            socket_reg.as_deref_mut(),
            decal_state.as_deref_mut(),
            rig_mut.as_deref_mut(),
        );
    }

    let mut bf_beams = VfxBeamAndRingBatchBuilder::default();
    let cam_fwd = Vec3::new(0.0, -0.75, -0.66).normalize();
    if let Some(pool) = gpu_pool.as_deref_mut() {
        runtime.active_instances.retain_mut(|inst| {
            step_vfx_instance(inst, dt, cam_fwd, &mut pool.burst_buffer, &mut bf_beams)
        });
    }
    if let Some(mesh_assets) = meshes.as_deref_mut() {
        if let Some(m) = mesh_assets.get_mut(&runtime.beam_batch_mesh) {
            bf_beams.write_into_mesh(m);
        }
    }

    let Some(pstate) = preview_state.as_deref_mut() else {
        return;
    };
    let (prev_fwd, prev_right, prev_up) = preview_cam_q
        .iter()
        .next()
        .map(|tf| {
            (
                tf.forward().as_vec3(),
                tf.right().as_vec3(),
                tf.up().as_vec3(),
            )
        })
        .unwrap_or((Vec3::new(0.0, -0.45, -0.89).normalize(), Vec3::X, Vec3::Y));

    let mut prev_beams = VfxBeamAndRingBatchBuilder::default();
    if let Some(mut eff_def) = resolve_preview_effect(pstate, mod_res.as_deref().map(|m| &m.0)) {
        eff_def.looping = pstate.loop_enabled;
        let sig = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            eff_def.id.hash(&mut h);
            eff_def.emitters.len().hash(&mut h);
            eff_def.duration_secs.to_bits().hash(&mut h);
            pstate.loop_enabled.hash(&mut h);
            h.finish()
        };

        let burst_requested = std::mem::replace(&mut pstate.trigger_burst, false);
        if runtime.last_preview_signature != sig
            || runtime.preview_instance.is_none()
            || burst_requested
        {
            runtime.last_preview_signature = sig;
            runtime.last_paused_scrub_secs = None;
            runtime.preview_burst_buffer.clear();
            if burst_requested {
                pstate.timeline_secs = 0.0;
            }
            runtime.preview_instance = Some(ActiveDataDrivenVfxInstance {
                effect_id: eff_def.id.clone(),
                origin: Vec3::new(0.0, 0.95, 0.0),
                target_pos: Vec3::new(1.85, 1.05, 0.0),
                forward: Vec3::new(0.35, 0.92, 0.12).normalize(),
                age_secs: 0.0,
                duration_secs: eff_def.duration_secs.max(0.1),
                looping: pstate.loop_enabled,
                attached_entity_id: None,
                attached_projectile_id: None,
                seed: 0x517C_C1B7_2722_0A95,
                emitters: eff_def
                    .emitters
                    .iter()
                    .cloned()
                    .map(ActiveEmitterRuntime::from_def)
                    .collect(),
            });
        }

        let rt = &mut *runtime;
        let prev_part_handle = rt.preview_particle_mesh.clone();
        if pstate.playback_paused {
            let target_t = pstate
                .timeline_secs
                .clamp(0.0, eff_def.duration_secs.max(0.1));
            if rt
                .last_paused_scrub_secs
                .map(|prev| (prev - target_t).abs() > 1e-3)
                .unwrap_or(true)
            {
                rt.last_paused_scrub_secs = Some(target_t);
                rt.preview_burst_buffer.clear();
                if let Some(inst) = rt.preview_instance.as_mut() {
                    inst.age_secs = 0.0;
                    for em in &mut inst.emitters {
                        em.reset_cycle();
                    }
                    let step = 1.0 / 60.0;
                    let mut sim_t = 0.0;
                    while sim_t < target_t {
                        let s_dt = (target_t - sim_t).min(step);
                        prev_beams = VfxBeamAndRingBatchBuilder::default();
                        let _ = step_vfx_instance(
                            inst,
                            s_dt,
                            prev_fwd,
                            &mut rt.preview_burst_buffer,
                            &mut prev_beams,
                        );
                        if let Some(mesh_assets) = meshes.as_deref_mut() {
                            if let Some(pm) = mesh_assets.get_mut(&prev_part_handle) {
                                rt.preview_burst_buffer
                                    .step_and_build_billboards(s_dt, None, prev_right, prev_up, pm);
                            }
                        }
                        sim_t += step;
                    }
                }
            } else if let Some(inst) = rt.preview_instance.as_mut() {
                let _ = step_vfx_instance(
                    inst,
                    0.0,
                    prev_fwd,
                    &mut rt.preview_burst_buffer,
                    &mut prev_beams,
                );
                if let Some(mesh_assets) = meshes.as_deref_mut() {
                    if let Some(pm) = mesh_assets.get_mut(&prev_part_handle) {
                        rt.preview_burst_buffer
                            .step_and_build_billboards(0.0, None, prev_right, prev_up, pm);
                    }
                }
            }
        } else {
            rt.last_paused_scrub_secs = None;
            if let Some(inst) = rt.preview_instance.as_mut() {
                let alive = step_vfx_instance(
                    inst,
                    dt,
                    prev_fwd,
                    &mut rt.preview_burst_buffer,
                    &mut prev_beams,
                );
                pstate.timeline_secs = inst.age_secs;
                if !alive {
                    rt.preview_instance = None;
                }
            }
            if let Some(mesh_assets) = meshes.as_deref_mut() {
                if let Some(pm) = mesh_assets.get_mut(&prev_part_handle) {
                    rt.preview_burst_buffer
                        .step_and_build_billboards(dt, None, prev_right, prev_up, pm);
                }
            }
        }
    } else {
        runtime.preview_instance = None;
        runtime.preview_burst_buffer.clear();
        if let Some(mesh_assets) = meshes.as_deref_mut() {
            if let Some(pm) = mesh_assets.get_mut(&runtime.preview_particle_mesh) {
                runtime
                    .preview_burst_buffer
                    .step_and_build_billboards(0.0, None, prev_right, prev_up, pm);
            }
        }
    }

    if let Some(mesh_assets) = meshes.as_deref_mut() {
        if let Some(bm) = mesh_assets.get_mut(&runtime.preview_beam_mesh) {
            prev_beams.write_into_mesh(bm);
        }
    }
}
