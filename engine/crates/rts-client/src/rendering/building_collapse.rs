use super::entity_articulation::{discover_articulated_descendants, ArticulatedNodeFilter};
use super::preview_stage::PreviewStageEntity as PreviewEnt;
use super::{
    ArticulatedPartKind as PartKind, BuildingConstructionRig, CachedFogVisibility,
    DamageVisualTier, EntityAnimController, MapEnvironmentEntity as MapEnv, SimVisualEntity,
    SyncedGltfSceneVisuals, UnitPbrTeamMaterial,
};
use crate::vfx::{GpuParticleBurstBuffer, GpuParticleInstance, GpuParticlePoolResources};
use crate::SimWorldResource;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_simulation::world::{SimWorld, TileVisibility};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollapseTierKind {
    #[default]
    CrownTech,
    UpperShell,
    CoreFrame,
    Flap,
    HullPad,
}

#[derive(Debug, Clone, Copy)]
pub struct CollapseSubPart {
    pub entity: Entity,
    pub tier: CollapseTierKind,
    pub rest_translation: Vec3,
    pub rest_rotation: Quat,
    pub max_pitch_rad: f32,
    pub max_roll_rad: f32,
    pub drift_xz: Vec2,
}

#[derive(Component, Debug, Clone, Default)]
pub struct BuildingCollapseVisual {
    pub sim_id: u64,
    pub elapsed: f32,
    pub progress: f32,
    pub duration: f32,
    pub base_y: f32,
    pub footprint: [u32; 2],
    pub seed: u64,
    pub frozen: bool,
    pub initialized_fx: bool,
    pub next_smoke_burst: f32,
    pub parts: Option<Vec<CollapseSubPart>>,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CollapseDebrisChunk {
    pub velocity: Vec3,
    pub angular_vel: Vec3,
    pub ground_y: f32,
    pub age: f32,
    pub lifetime: f32,
    pub base_scale: Vec3,
    pub bounces: u8,
    pub frozen: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct ScorchedBuildingRubble {
    pub age: f32,
    pub lifetime: f32,
    pub base_y: f32,
}

#[derive(Component, Debug, Clone, Default)]
pub struct VehicleWreckVisual {
    pub sim_id: u64,
    pub elapsed: f32,
    pub duration: f32,
    pub base_y: f32,
    pub seed: u64,
    pub frozen: bool,
    pub materials_blackened: bool,
    pub hull_base_rot: Quat,
    pub turret_nodes: Vec<(Entity, Vec3)>,
}

#[derive(Resource, Default)]
pub struct BuildingCollapseAssets {
    pub initialized: bool,
    pub girder_mesh: Handle<Mesh>,
    pub slab_mesh: Handle<Mesh>,
    pub rubble_pad_mesh: Handle<Mesh>,
    pub girder_mat: Handle<StandardMaterial>,
    pub concrete_mat: Handle<StandardMaterial>,
    pub rubble_mat: Handle<StandardMaterial>,
    pub ember_mat: Handle<StandardMaterial>,
}

impl BuildingCollapseAssets {
    pub fn ensure(&mut self, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) {
        if self.initialized {
            return;
        }
        let mk = |mats: &mut Assets<StandardMaterial>, c: [f32; 3], m: f32, r: f32, e: [f32; 3]| {
            let mut sm = StandardMaterial::from_color(Color::srgb(c[0], c[1], c[2]));
            sm.emissive = LinearRgba::new(e[0], e[1], e[2], 1.0);
            (sm.metallic, sm.perceptual_roughness) = (m, r);
            mats.add(sm)
        };
        self.girder_mesh = meshes.add(Cuboid::new(0.16, 0.14, 1.32));
        self.slab_mesh = meshes.add(Cuboid::new(0.56, 0.20, 0.46));
        self.rubble_pad_mesh = meshes.add(Cuboid::new(1.0, 0.12, 1.0));
        self.girder_mat = mk(mats, [0.16, 0.15, 0.15], 0.78, 0.52, [0.45, 0.12, 0.02]);
        self.concrete_mat = mk(mats, [0.26, 0.24, 0.22], 0.08, 0.92, [0.0; 3]);
        self.rubble_mat = mk(mats, [0.12, 0.11, 0.10], 0.10, 0.96, [0.0; 3]);
        self.ember_mat = mk(mats, [0.95, 0.34, 0.08], 0.05, 0.65, [2.8, 0.65, 0.08]);
        self.initialized = true;
    }
}

#[inline]
fn hash01(seed: u64) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (((x ^ (x >> 31)) & 0x00FF_FFFF) as f32) / (0x0100_0000 as f32)
}

#[inline]
fn sgn(seed: u64) -> f32 {
    if hash01(seed) > 0.5 {
        1.0
    } else {
        -1.0
    }
}

#[inline]
pub fn compute_collapse_clip_progress(progress_01: f32) -> f32 {
    (1.0 - progress_01.clamp(0.0, 1.0) * 0.85).clamp(0.08, 0.92)
}

#[inline]
pub fn compute_collapse_tier_offset_and_rot(
    p: &CollapseSubPart,
    el: f32,
    p01: f32,
    wh: bool,
) -> (Vec3, Quat) {
    let (dly, gm, re) = match p.tier {
        CollapseTierKind::CrownTech => (0.00, 1.00, 1.45),
        CollapseTierKind::UpperShell => (0.06, 0.92, 1.30),
        CollapseTierKind::CoreFrame => (0.13, 0.78, 1.15),
        CollapseTierKind::Flap => (0.04, 0.58, 1.50),
        CollapseTierKind::HullPad if wh => (0.02, 0.76, 1.25),
        CollapseTierKind::HullPad => (0.32, 0.52, 0.85),
    };
    let te = (el - dly).max(0.0);
    let dy = -3.8 * te * te * gm;
    let rt = (p01 * re).clamp(0.0, 1.0);
    let sm = rt * rt * (3.0 - 2.0 * rt);
    let shx = if p01 < 0.65 {
        (el * 44.0).sin() * (1.0 - p01) * 0.045
    } else {
        0.0
    };
    let off = Vec3::new(p.drift_xz.x * sm + shx, dy, p.drift_xz.y * sm);
    let tilt = Quat::from_euler(
        EulerRot::YXZ,
        0.0,
        p.max_pitch_rad * sm,
        p.max_roll_rad * sm,
    );
    (
        p.rest_translation + off,
        (p.rest_rotation * tilt).normalize(),
    )
}

#[inline]
pub fn compute_rubble_sink_y(base_y: f32, age: f32, lifetime: f32) -> f32 {
    let s0 = (lifetime - 2.0).max(0.0);
    if age <= s0 {
        base_y
    } else {
        base_y - 0.68 * ((age - s0) * 0.5).clamp(0.0, 1.0).powi(2)
    }
}

#[inline]
pub fn compute_turret_pop_pose(rest: Vec3, el: f32, dur: f32, seed: u64) -> (Vec3, Quat) {
    let u = (el / dur.max(0.1)).clamp(0.0, 1.0);
    let (dy, ang) = (6.18 * u * (1.0 - 0.84 * u), hash01(seed) * TAU);
    let hz = (0.75 + hash01(seed.wrapping_add(3)) * 0.55) * u;
    let (sx, sz) = (sgn(seed.wrapping_add(7)), sgn(seed.wrapping_add(11)));
    let pos = rest + Vec3::new(ang.cos() * hz, dy, ang.sin() * hz);
    (
        pos,
        Quat::from_euler(EulerRot::YXZ, sx * 2.6 * u, sx * 2.15 * u, sz * 2.85 * u),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn try_attach_combat_collapse_or_wreck(
    commands: &mut Commands,
    bevy_ent: Entity,
    vis: &mut SimVisualEntity,
    base_y: f32,
    hull_rot: Quat,
    world: &SimWorld,
    loaded: &LoadedMod,
    ch_q: &Query<&Children>,
    tq: &Query<(&Name, &mut Transform), ArticulatedNodeFilter>,
) -> bool {
    let (mut sold, mut dead) = (false, false);
    for ev in &world.destruction_events {
        if ev.entity_id == vis.sim_id {
            (sold, dead) = (sold || ev.is_sold, dead || !ev.is_sold);
        }
    }
    if vis.deconstruct_progress.is_none() && vis.building_footprint.is_some() {
        let b_opt = loaded.buildings.get(&vis.entity_type_id);
        let undep = !dead && b_opt.is_some_and(|b| b.transforms_into_unit.is_some());
        if sold || undep {
            vis.deconstruct_progress = Some(vis.spawn_progress.clamp(0.25, 1.0));
        } else if dead {
            vis.deconstruct_progress = Some(0.92);
            let mut col = BuildingCollapseVisual::default();
            (col.sim_id, col.duration, col.base_y) = (vis.sim_id, 1.35, base_y);
            col.footprint = vis.building_footprint.unwrap_or([2, 2]);
            col.seed = vis.sim_id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5_5A5A;
            commands.entity(bevy_ent).insert(col);
            return true;
        }
    }
    if !vis.is_infantry && vis.building_footprint.is_none() && dead {
        if vis.cached_articulated_parts.is_none() {
            if let Some(f) = discover_articulated_descendants(bevy_ent, ch_q, tq) {
                vis.has_3d_turret_node = f
                    .iter()
                    .any(|p| matches!(p.kind, PartKind::Turret | PartKind::Barrel));
                vis.cached_articulated_parts = Some(f);
            }
        }
        let t_nodes: Vec<(Entity, Vec3)> = vis
            .cached_articulated_parts
            .as_deref()
            .into_iter()
            .flatten()
            .filter(|p| {
                p.kind == PartKind::Turret || (p.kind == PartKind::Barrel && !p.parent_is_turret)
            })
            .map(|p| (p.entity, p.rest_translation))
            .collect();
        if !t_nodes.is_empty() {
            let mut wr = VehicleWreckVisual::default();
            (wr.sim_id, wr.duration, wr.base_y) = (vis.sim_id, 0.85, base_y);
            wr.seed = vis.sim_id.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ 0x517C_C1B7;
            (wr.hull_base_rot, wr.turret_nodes) = (hull_rot, t_nodes);
            commands.entity(bevy_ent).insert(wr);
            return true;
        }
    }
    if vis.is_infantry && vis.death_anim_progress.is_none() && dead {
        vis.death_anim_progress = Some(0.0);
    }
    false
}

type ChildFilter = (
    Without<SimVisualEntity>,
    Without<BuildingCollapseVisual>,
    Without<VehicleWreckVisual>,
    Without<BuildingConstructionRig>,
    Without<CollapseDebrisChunk>,
    Without<ScorchedBuildingRubble>,
    Without<PreviewEnt>,
);
type WreckFilter = (
    Without<BuildingCollapseVisual>,
    Without<BuildingConstructionRig>,
    Without<CollapseDebrisChunk>,
    Without<ScorchedBuildingRubble>,
    Without<PreviewEnt>,
);

fn classify_node(base: &str) -> Option<(CollapseTierKind, u64, f32)> {
    match base {
        "CrownTech" | "DominatorCore" | "PsiCrown" | "AmplifierCrown" | "BeaconCrown"
        | "PrismHead" | "ToroidHead" => Some((CollapseTierKind::CrownTech, 11, 0.72)),
        "UpperShell" | "CraneBoom" | "Turret" => Some((CollapseTierKind::UpperShell, 29, 0.48)),
        "CoreFrame" | "FactoryDoor" | "BioCore" => Some((CollapseTierKind::CoreFrame, 47, 0.28)),
        s if s.starts_with("Flap_") => Some((CollapseTierKind::Flap, 71, 0.35)),
        "Hull" => Some((CollapseTierKind::HullPad, 97, 0.14)),
        _ => None,
    }
}

fn discover_subparts(
    root: Entity,
    seed: u64,
    ch_q: &Query<&Children>,
    cq: &Query<(Option<&Name>, &mut Transform), ChildFilter>,
) -> Vec<CollapseSubPart> {
    let (mut out, mut stack) = (Vec::new(), vec![root]);
    while let Some(curr) = stack.pop() {
        if let Ok(children) = ch_q.get(curr) {
            for &child in children.iter() {
                if let Ok((Some(name), tf)) = cq.get(child) {
                    if let Some((tier, salt, dm)) =
                        classify_node(name.as_str().trim_end_matches("_LOD1"))
                    {
                        let s = seed.wrapping_add(salt);
                        let (sp, sr) = (sgn(s + 5), sgn(s + 7));
                        let mp = sp * (14.0 + hash01(s) * 18.0).to_radians();
                        let mr = sr * (14.0 + hash01(s + 3) * 18.0).to_radians();
                        out.push(CollapseSubPart {
                            entity: child,
                            tier,
                            rest_translation: tf.translation,
                            rest_rotation: tf.rotation,
                            max_pitch_rad: mp,
                            max_roll_rad: mr,
                            drift_xz: Vec2::new(sr * dm, sp * dm * 0.8),
                        });
                    }
                }
                stack.push(child);
            }
        }
    }
    out
}

fn emit_smoke(
    burst: &mut GpuParticleBurstBuffer,
    c: Vec3,
    col: &BuildingCollapseVisual,
    salt: u64,
) {
    let (by, seed) = (col.base_y, col.seed.wrapping_add(salt));
    let (hw, hd) = (
        (col.footprint[0].max(1) as f32) * 0.85,
        (col.footprint[1].max(1) as f32) * 0.85,
    );
    for i in 0..10u64 {
        let s = seed.wrapping_add(i * 41);
        let (ang, fire, spd) = (hash01(s) * TAU, i % 3 == 0, 1.2 + hash01(s + 4) * 2.0);
        let px = c.x + (hash01(s + 1) - 0.5) * hw * 1.3;
        let pz = c.z + (hash01(s + 3) - 0.5) * hd * 1.3;
        let inst = GpuParticleInstance {
            position: Vec3::new(px, by + 0.35 + hash01(s + 2) * 1.4, pz),
            velocity: Vec3::new(ang.cos() * spd, 1.8 + hash01(s + 5) * 2.6, ang.sin() * spd),
            age: 0.0,
            lifetime: if fire { 0.55 } else { 1.35 },
            gravity: if fire { 5.5 } else { -0.9 },
            start_size: if fire { 0.48 } else { 0.68 },
            end_size: if fire { 0.14 } else { 1.85 },
            color_rgba: if fire {
                [1.0, 0.48, 0.12, 0.94]
            } else {
                [0.16, 0.16, 0.18, 0.72]
            },
        };
        burst.emit_with_drag(inst, 0.55);
    }
}

fn spawn_fx(
    commands: &mut Commands,
    assets: &BuildingCollapseAssets,
    burst: Option<&mut GpuParticleBurstBuffer>,
    c: Vec3,
    col: &BuildingCollapseVisual,
) {
    let (by, fp, seed) = (col.base_y, col.footprint, col.seed);
    let (hw, hd) = ((fp[0].max(1) as f32) * 0.88, (fp[1].max(1) as f32) * 0.88);
    let n = (4 + fp[0].max(fp[1]) * 2).clamp(6, 10) as u64;
    for i in 0..n {
        let s = seed.wrapping_add(i * 131);
        let (ang, grd) = ((i as f32 / n as f32) * TAU + hash01(s) * 0.55, i % 2 == 0);
        let (ov, uv) = (2.8 + hash01(s + 1) * 3.4, 4.4 + hash01(s + 2) * 3.4);
        let pos = Vec3::new(
            c.x + ang.cos() * hw * 0.45,
            by + 1.15 + hash01(s + 3) * 1.55,
            c.z + ang.sin() * hd * 0.45,
        );
        let sc = if grd {
            Vec3::new(1.0, 1.0, 0.85 + hash01(s + 4) * 0.55)
        } else {
            Vec3::splat(0.80 + hash01(s + 4) * 0.50)
        };
        let (gm, sm) = (assets.girder_mesh.clone(), assets.slab_mesh.clone());
        let (gmat, cmat) = (assets.girder_mat.clone(), assets.concrete_mat.clone());
        let (mh, mth) = if grd { (gm, gmat) } else { (sm, cmat) };
        let (ax, ay, az) = (
            (hash01(s + 5) - 0.5) * 11.0,
            (hash01(s + 6) - 0.5) * 9.0,
            (hash01(s + 7) - 0.5) * 11.0,
        );
        let ch = CollapseDebrisChunk {
            velocity: Vec3::new(ang.cos() * ov, uv, ang.sin() * ov),
            angular_vel: Vec3::new(ax, ay, az),
            ground_y: by,
            lifetime: 1.75 + hash01(s + 8) * 0.55,
            base_scale: sc,
            ..default()
        };
        let mut tf = Transform::from_translation(pos).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            ang,
            0.4,
            0.3,
        ));
        tf.scale = sc;
        commands.spawn(((MapEnv, ch), (Mesh3d(mh), MeshMaterial3d(mth), tf)));
    }

    let ptf =
        Transform::from_xyz(c.x, by + 0.04, c.z).with_scale(Vec3::new(hw * 1.82, 1.0, hd * 1.82));
    let rub = ScorchedBuildingRubble {
        age: 0.0,
        lifetime: 16.0,
        base_y: by,
    };
    let pm = (
        Mesh3d(assets.rubble_pad_mesh.clone()),
        MeshMaterial3d(assets.rubble_mat.clone()),
        ptf,
    );
    commands.spawn(((MapEnv, rub), pm)).with_children(|p| {
        let etf = Transform::from_xyz(0.0, 0.55, 0.0).with_scale(Vec3::new(0.36, 0.45, 0.36));
        p.spawn((
            Mesh3d(assets.rubble_pad_mesh.clone()),
            MeshMaterial3d(assets.ember_mat.clone()),
            etf,
        ));
        for j in 0..5u64 {
            let rs = seed.wrapping_add(900 + j * 53);
            let (gm, sm) = (assets.girder_mesh.clone(), assets.slab_mesh.clone());
            let (gmat, cmat) = (assets.girder_mat.clone(), assets.concrete_mat.clone());
            let (mh, mth) = if j % 2 == 0 { (gm, gmat) } else { (sm, cmat) };
            let (yaw, pit) = (hash01(rs + 2) * TAU, 0.32 + hash01(rs + 3) * 0.45);
            let mut rtf = Transform::from_xyz(
                (hash01(rs) - 0.5) * 0.68,
                1.6,
                (hash01(rs + 1) - 0.5) * 0.68,
            );
            rtf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pit, 0.22);
            rtf.scale = Vec3::new(0.35 / hw.max(0.8), 2.2, 0.35 / hd.max(0.8));
            p.spawn((Mesh3d(mh), MeshMaterial3d(mth), rtf));
        }
    });

    if let Some(b) = burst {
        emit_smoke(b, c, col, 0);
    }
}

type Sv = SimVisualEntity;
type Rig = BuildingConstructionRig;
type ColVis = BuildingCollapseVisual;
type WrkVis = VehicleWreckVisual;
type DebChk = CollapseDebrisChunk;
type RubVis = ScorchedBuildingRubble;
type AnimCtrl = EntityAnimController;
type SyncVis = SyncedGltfSceneVisuals;

#[derive(SystemParam)]
pub struct CollapseStepRes<'w> {
    pub time: Res<'w, Time>,
    pub assets: ResMut<'w, BuildingCollapseAssets>,
    pub cached_fow: Option<Res<'w, CachedFogVisibility>>,
    pub sim_res: Option<Res<'w, SimWorldResource>>,
    pub meshes: Option<ResMut<'w, Assets<Mesh>>>,
    pub std_mats: Option<ResMut<'w, Assets<StandardMaterial>>>,
    pub unit_pbr_mats: Option<ResMut<'w, Assets<UnitPbrTeamMaterial>>>,
    pub gpu_pool: Option<ResMut<'w, GpuParticlePoolResources>>,
}

fn is_explored(res: &CollapseStepRes, p: Vec3) -> bool {
    match (res.cached_fow.as_deref(), res.sim_res.as_deref()) {
        (Some(f), Some(s)) if f.fog_enabled => {
            f.world_pos_visibility(&s.0, p.to_array()) != TileVisibility::Unexplored
        }
        _ => true,
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn step_building_collapses_and_wrecks(
    mut commands: Commands,
    mut res: CollapseStepRes,
    mut collapse_q: Query<
        (
            Entity,
            &mut Sv,
            &mut ColVis,
            &Transform,
            &mut Visibility,
            Option<&mut AnimCtrl>,
            Option<&mut SyncVis>,
        ),
        (Without<WrkVis>, Without<Rig>, Without<PreviewEnt>),
    >,
    mut wreck_q: Query<
        (
            Entity,
            &mut WrkVis,
            &mut Transform,
            &mut Visibility,
            Option<&SyncVis>,
        ),
        WreckFilter,
    >,
    mut rigs_q: Query<
        (&Rig, &mut Visibility),
        (
            Without<Sv>,
            Without<ColVis>,
            Without<WrkVis>,
            Without<PreviewEnt>,
        ),
    >,
    children_q: Query<&Children>,
    mut child_tf_q: Query<(Option<&Name>, &mut Transform), ChildFilter>,
    mut anim_q: Query<&mut AnimationPlayer, Without<PreviewEnt>>,
    mut debris_q: Query<
        (Entity, &mut DebChk, &mut Transform),
        (Without<Sv>, Without<RubVis>, Without<PreviewEnt>),
    >,
    mut rubble_q: Query<
        (Entity, &mut RubVis, &mut Transform),
        (Without<Sv>, Without<DebChk>, Without<PreviewEnt>),
    >,
) {
    let dt = res.time.delta_secs();
    if let (Some(m), Some(sm)) = (res.meshes.as_deref_mut(), res.std_mats.as_deref_mut()) {
        res.assets.ensure(m, sm);
    }

    let mut active_ids = Vec::new();
    for (ent, mut vis, mut col, root_tf, mut r_vis, ctrl_opt, synced_opt) in &mut collapse_q {
        active_ids.push(col.sim_id);
        if *r_vis == Visibility::Hidden && is_explored(&res, root_tf.translation) {
            *r_vis = Visibility::Inherited;
        }
        if let Some(mut ctrl) = ctrl_opt {
            (ctrl.articulated_buildup, ctrl.anim_cache_resolved) = (true, true);
            if let Some(players) = ctrl.anim_players.take() {
                for pe in players {
                    if let Ok(mut pl) = anim_q.get_mut(pe) {
                        pl.stop_all();
                    }
                    commands.entity(pe).remove::<AnimationPlayer>();
                }
                ctrl.anim_players = Some(Vec::new());
            }
        }
        if !col.initialized_fx && res.assets.initialized {
            col.initialized_fx = true;
            let b_opt = res.gpu_pool.as_deref_mut().map(|p| &mut p.burst_buffer);
            spawn_fx(&mut commands, &res.assets, b_opt, root_tf.translation, &col);
        }
        if col.parts.is_none() {
            let disc = discover_subparts(ent, col.seed, &children_q, &child_tf_q);
            if !disc.is_empty() {
                col.parts = Some(disc);
            }
        }
        if !col.frozen {
            col.elapsed += dt;
        }
        col.progress = (col.elapsed / col.duration.max(0.1)).clamp(0.0, 1.0);
        if !col.frozen && col.progress >= 1.0 {
            commands.entity(ent).despawn_recursive();
            continue;
        }

        let clip_p = compute_collapse_clip_progress(col.progress);
        vis.deconstruct_progress = Some(clip_p);
        if let Some(mut synced) = synced_opt {
            if synced.cached_damage_tier != DamageVisualTier::Red25 {
                (synced.cached_damage_tier, synced.applied_pbr_key) =
                    (DamageVisualTier::Red25, None);
            }
            if let Some(ref mut ext) = res.unit_pbr_mats {
                for hd in &synced.active_construct_handles {
                    if let Some(mat) = ext.get_mut(hd) {
                        mat.extension.params.damage_and_fx.x = 1.0;
                        mat.extension.params.construction_params =
                            Vec4::new(clip_p, col.base_y, -100.0, -2.0);
                    }
                }
            }
        }
        if let Some(ref parts) = col.parts {
            let wh = !parts.iter().any(|p| p.tier != CollapseTierKind::HullPad);
            let inv_s = 1.0 / root_tf.scale.max_element().max(0.5);
            for part in parts {
                if let Ok((_, mut tf)) = child_tf_q.get_mut(part.entity) {
                    let (wp, rot) =
                        compute_collapse_tier_offset_and_rot(part, col.elapsed, col.progress, wh);
                    tf.translation = part.rest_translation + (wp - part.rest_translation) * inv_s;
                    tf.rotation = rot;
                }
            }
        }
        if !col.frozen && col.elapsed >= col.next_smoke_burst {
            col.next_smoke_burst = col.elapsed + 0.22;
            if let Some(pool) = res.gpu_pool.as_deref_mut() {
                emit_smoke(
                    &mut pool.burst_buffer,
                    root_tf.translation,
                    &col,
                    (col.elapsed * 100.0) as u64,
                );
            }
        }
    }

    for (rig, mut rv) in &mut rigs_q {
        if active_ids.contains(&rig.sim_id) && *rv != Visibility::Hidden {
            *rv = Visibility::Hidden;
        }
    }

    for (ent, mut wr, mut htf, mut w_vis, synced_opt) in &mut wreck_q {
        if !wr.frozen {
            wr.elapsed += dt;
        }
        let u = (wr.elapsed / wr.duration.max(0.1)).clamp(0.0, 1.0);
        if !wr.frozen && u >= 1.0 {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        if *w_vis == Visibility::Hidden && is_explored(&res, htf.translation) {
            *w_vis = Visibility::Inherited;
        }
        if !wr.materials_blackened && res.assets.initialized {
            if let Some(synced) = synced_opt {
                for &(me, _) in &synced.mesh_entities {
                    let rmat = MeshMaterial3d(res.assets.rubble_mat.clone());
                    commands
                        .entity(me)
                        .remove::<MeshMaterial3d<UnitPbrTeamMaterial>>()
                        .insert(rmat);
                }
                wr.materials_blackened = true;
            }
        }
        let sink = if u > 0.72 {
            -0.38 * ((u - 0.72) / 0.28).powi(2)
        } else {
            0.0
        };
        let jolt = (1.0 - u) * (wr.elapsed * 26.0).sin() * 0.06;
        htf.translation.y = wr.base_y + sink;
        let wrot = Quat::from_euler(EulerRot::YXZ, 0.0, -0.08 * u + jolt, 0.09 * u);
        htf.rotation = (wr.hull_base_rot * wrot).normalize();
        let inv_ws = 1.0 / htf.scale.max_element().max(0.4);
        for &(te, rpos) in &wr.turret_nodes {
            if let Ok((_, mut ttf)) = child_tf_q.get_mut(te) {
                let (wp, rot) = compute_turret_pop_pose(rpos, wr.elapsed, wr.duration, wr.seed);
                ttf.translation = rpos + (wp - rpos) * inv_ws;
                ttf.rotation = rot;
            }
        }
    }

    for (ent, mut ch, mut tf) in &mut debris_q {
        if ch.frozen {
            continue;
        }
        ch.age += dt;
        if ch.age >= ch.lifetime {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        ch.velocity.y -= 13.5 * dt;
        tf.translation += ch.velocity * dt;
        if tf.translation.y <= ch.ground_y + 0.10 && ch.velocity.y < 0.0 {
            tf.translation.y = ch.ground_y + 0.10;
            ch.velocity.y = (-ch.velocity.y * 0.36).min(3.2);
            (ch.velocity.x, ch.velocity.z) = (ch.velocity.x * 0.64, ch.velocity.z * 0.64);
            (ch.angular_vel, ch.bounces) = (ch.angular_vel * 0.55, ch.bounces.saturating_add(1));
        }
        if ch.bounces < 3 {
            let av = ch.angular_vel * dt;
            tf.rotation =
                (tf.rotation * Quat::from_euler(EulerRot::YXZ, av.y, av.x, av.z)).normalize();
        }
        tf.scale = ch.base_scale * ((ch.lifetime - ch.age).clamp(0.0, 0.32) / 0.32).max(0.05);
    }

    for (ent, mut rub, mut tf) in &mut rubble_q {
        rub.age += dt;
        if rub.age >= rub.lifetime {
            commands.entity(ent).despawn_recursive();
            continue;
        }
        tf.translation.y = compute_rubble_sink_y(rub.base_y, rub.age, rub.lifetime) + 0.04;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_gravity_tilt_turret_pop_and_rubble_sink_match_ra2_contracts() {
        let part = CollapseSubPart {
            entity: Entity::PLACEHOLDER,
            tier: CollapseTierKind::CrownTech,
            rest_translation: Vec3::new(0.0, 2.5, 0.0),
            rest_rotation: Quat::IDENTITY,
            max_pitch_rad: 24.0_f32.to_radians(),
            max_roll_rad: -18.0_f32.to_radians(),
            drift_xz: Vec2::new(0.5, 0.4),
        };
        let (pos_mid, rot_mid) = compute_collapse_tier_offset_and_rot(&part, 0.52, 0.385, false);
        assert!((pos_mid.y - (2.5 - 3.8 * 0.52 * 0.52)).abs() < 1e-3);
        assert!(rot_mid.angle_between(Quat::IDENTITY) > 0.15);
        assert!((compute_collapse_clip_progress(0.0) - 0.92).abs() < 1e-5);
        assert!((compute_collapse_clip_progress(1.0) - 0.15).abs() < 1e-5);
        let (pop_mid, _) = compute_turret_pop_pose(Vec3::new(0.0, 0.8, 0.0), 0.44, 0.85, 42);
        assert!(pop_mid.y - 0.8 > 1.72 && pop_mid.y - 0.8 < 2.05);
        assert!((compute_rubble_sink_y(1.0, 10.0, 16.0) - 1.0).abs() < 1e-5);
        assert!(compute_rubble_sink_y(1.0, 15.0, 16.0) < 0.88);
        assert!(compute_rubble_sink_y(1.0, 16.0, 16.0) < 0.35);
    }
}
