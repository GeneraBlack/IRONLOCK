use super::entity_articulation::{upgrade_scene_hierarchy_to_unit_pbr, ScenePbrUpgradeCtx};
use super::preview_stage::{PreviewStageCamera, PreviewStageEntity as PreviewEnt};
use super::world_overlays::{PropInstanceKey, PropSceneVisualEntity};
use super::{
    resolve_mod_asset_path, CachedFogVisibility, DamageVisualTier, FogOfWarOverlayImage,
    MapEnvironmentEntity as MapEnv, MicroDetailTextureResource, Ra2EntityVisualMode,
    ShieldFresnelKind, SmoothFogTemporalState, UnitPbrTeamMaterial,
};
use crate::camera::RtsCameraRig;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::ecs::system::SystemParam;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{CrateKind, TeamMaskMode, VisualDef};
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::{EntityKind, SimWorld, TileVisibility};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::atomic::{AtomicU8, Ordering};

static LIVE_BRIDGE_VISUAL_MODE: AtomicU8 = AtomicU8::new(0);

pub const BRIDGE_INTACT_REL_PATH: &str = "assets/models/props/bridge_span_intact.glb";
pub const BRIDGE_DESTROYED_REL_PATH: &str = "assets/models/props/bridge_span_destroyed.glb";
pub const SUPPLY_CRATE_REL_PATH: &str = "assets/models/props/supply_crate.glb";

#[inline]
fn vis_if(c: bool) -> Visibility {
    if c {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

#[inline]
pub fn mod_has_3d_bridge_spans(loaded: &LoadedMod) -> bool {
    loaded.asset_exists(BRIDGE_INTACT_REL_PATH)
}

#[inline]
pub fn should_use_3d_gltf_bridges(loaded: &LoadedMod, mode: Ra2EntityVisualMode) -> bool {
    mode == Ra2EntityVisualMode::Full3dMeshes && mod_has_3d_bridge_spans(loaded)
}

pub fn sync_bridge_visual_mode_flag(visual_mode: Option<Res<Ra2EntityVisualMode>>) {
    let m = visual_mode.as_deref().copied().unwrap_or_default();
    let code = if m == Ra2EntityVisualMode::Full3dMeshes {
        1
    } else {
        2
    };
    LIVE_BRIDGE_VISUAL_MODE.store(code, Ordering::Relaxed);
}

#[inline]
pub fn is_3d_bridge_mode_active_for_overlays(loaded: &LoadedMod) -> bool {
    mod_has_3d_bridge_spans(loaded)
        && match LIVE_BRIDGE_VISUAL_MODE.load(Ordering::Relaxed) {
            1 => true,
            2 => false,
            _ => {
                let d = Ra2EntityVisualMode::default_for_mod_root(&loaded.root_dir);
                d == Ra2EntityVisualMode::Full3dMeshes
            }
        }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BridgeSpanKey {
    IntactSlice(bool, i32, i32, i32),
    DestroyedStub(usize, u8),
}

#[derive(Component, Debug, Clone)]
pub struct BridgeSpan3dVisual {
    pub key: BridgeSpanKey,
    pub is_destroyed: bool,
    pub meshes_upgraded: bool,
    pub material_handles: Vec<Handle<UnitPbrTeamMaterial>>,
}

#[derive(Component, Debug, Clone, Default)]
pub struct SupplyCrate3dVisual {
    pub crate_id: u64,
    pub is_credits: bool,
    pub meshes_upgraded: bool,
    pub material_handles: Vec<Handle<UnitPbrTeamMaterial>>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SupplyCratePulseRing;

#[derive(Component, Debug, Clone, Copy)]
pub struct TimedBombBeacon3dVisual {
    pub target_sim_id: u64,
}

#[derive(Resource, Default)]
pub struct BridgeAndCrateCache {
    pub loaded_mod_root: Option<std::path::PathBuf>,
    pub intact_bridge_scene: Option<Handle<Scene>>,
    pub destroyed_bridge_scene: Option<Handle<Scene>>,
    pub supply_crate_scene: Option<Handle<Scene>>,
    pub ring_mesh: Option<Handle<Mesh>>,
    pub gold_ring: Option<Handle<StandardMaterial>>,
    pub cyan_ring: Option<Handle<StandardMaterial>>,
    pub c4_box: Option<Handle<Mesh>>,
    pub c4_bulb: Option<Handle<Mesh>>,
    pub c4_mat: Option<Handle<StandardMaterial>>,
    pub strobe_mat: Option<Handle<StandardMaterial>>,
}

impl BridgeAndCrateCache {
    fn ensure_primitives(&mut self, ma: &mut Assets<Mesh>, sma: &mut Assets<StandardMaterial>) {
        if self.ring_mesh.is_some() {
            return;
        }
        let ring = |sma: &mut Assets<StandardMaterial>, c: [f32; 4], e: [f32; 3]| {
            let mut m = StandardMaterial::from_color(Color::srgba(c[0], c[1], c[2], c[3]));
            m.emissive = LinearRgba::new(e[0], e[1], e[2], 1.0);
            m.alpha_mode = AlphaMode::Blend;
            m.unlit = true;
            sma.add(m)
        };
        self.ring_mesh = Some(ma.add(Torus::new(0.48, 0.58)));
        self.gold_ring = Some(ring(sma, [1.0, 0.82, 0.20, 0.85], [3.2, 2.2, 0.35]));
        self.cyan_ring = Some(ring(sma, [0.22, 0.90, 1.0, 0.85], [0.45, 2.6, 3.4]));
        self.c4_box = Some(ma.add(Cuboid::new(0.34, 0.16, 0.24)));
        self.c4_bulb = Some(ma.add(Sphere::new(0.14)));
        let mut bm = StandardMaterial::from_color(Color::srgb(0.24, 0.28, 0.20));
        (bm.metallic, bm.perceptual_roughness) = (0.35, 0.65);
        self.c4_mat = Some(sma.add(bm));
        let mut sm = StandardMaterial::from_color(Color::srgb(1.0, 0.14, 0.10));
        sm.emissive = LinearRgba::new(8.5, 0.45, 0.20, 1.0);
        sm.unlit = true;
        self.strobe_mat = Some(sma.add(sm));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesiredBridgePlacement {
    pub key: BridgeSpanKey,
    pub is_destroyed: bool,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub visible: bool,
}

pub fn compute_desired_bridge_placements(
    world: &SimWorld,
    cached_fow: &CachedFogVisibility,
) -> Vec<DesiredBridgePlacement> {
    let mut out = Vec::new();
    let mut push = |key, is_destroyed, position, rotation, scale, visible| {
        out.push(DesiredBridgePlacement {
            key,
            is_destroyed,
            position,
            rotation,
            scale,
            visible,
        });
    };
    let (ts, by) = (
        world.grid.tile_size.max(0.5),
        CollisionGrid::BRIDGE_DECK_Y - 0.90,
    );
    let mut visited: BTreeSet<[i32; 2]> = BTreeSet::new();
    let is_land = |t: [i32; 2]| {
        world.grid.in_bounds(t)
            && !world.grid.water_tiles.contains(&t)
            && !world.grid.bridge_tiles.contains(&t)
    };

    for &seed in &world.grid.bridge_tiles {
        if !visited.insert(seed) {
            continue;
        }
        let (mut comp, mut q) = (Vec::new(), VecDeque::from([seed]));
        let (mut x0, mut x1, mut z0, mut z1) = (seed[0], seed[0], seed[1], seed[1]);
        while let Some(cur) = q.pop_front() {
            comp.push(cur);
            (x0, x1) = (x0.min(cur[0]), x1.max(cur[0]));
            (z0, z1) = (z0.min(cur[1]), z1.max(cur[1]));
            for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                let nb = [cur[0] + dx, cur[1] + dz];
                if world.grid.bridge_tiles.contains(&nb) && visited.insert(nb) {
                    q.push_back(nb);
                }
            }
        }
        let cset: BTreeSet<[i32; 2]> = comp.iter().copied().collect();
        let (mut sx, mut sz) = (i32::from(x1 - x0 >= z1 - z0), i32::from(z1 - z0 > x1 - x0));
        for &t in &comp {
            for (dx, dz, ew) in [(-1, 0, true), (1, 0, true), (0, -1, false), (0, 1, false)] {
                let nb = [t[0] + dx, t[1] + dz];
                if !cset.contains(&nb) {
                    let l = i32::from(is_land(nb)) * 3;
                    let w = i32::from(world.grid.water_tiles.contains(&nb)) * 2;
                    if ew {
                        (sx, sz) = (sx + l, sz + w);
                    } else {
                        (sz, sx) = (sz + l, sx + w);
                    }
                }
            }
        }
        let ax = sx >= sz;
        let step = if ts <= 1.05 { 2 } else { 1 };
        let (p0, p1) = if ax { (x0, x1) } else { (z0, z1) };
        let rot = Quat::from_rotation_y(if ax { FRAC_PI_2 } else { 0.0 });
        let mut p = p0;
        while p <= p1 {
            let pe = (p + step - 1).min(p1);
            let (mut l0, mut l1) = (i32::MAX, i32::MIN);
            for &[tx, tz] in &comp {
                let (pri, lat) = if ax { (tx, tz) } else { (tz, tx) };
                if pri >= p && pri <= pe {
                    (l0, l1) = (l0.min(lat), l1.max(lat));
                }
            }
            if l0 <= l1 {
                let (ta, tb) = if ax {
                    ([p, l0], [pe, l1])
                } else {
                    ([l0, p], [l1, pe])
                };
                let (wa, wb) = (world.grid.tile_to_world(ta), world.grid.tile_to_world(tb));
                let sl = (((pe - p + 1) as f32 * ts) * 0.5).clamp(0.45, 2.0);
                let sw = (((l1 - l0 + 1) as f32 * ts) * 0.5).clamp(0.55, 2.4);
                let vis = (p..=pe).any(|pi| {
                    (l0..=l1).any(|li| {
                        let t = if ax { [pi, li] } else { [li, pi] };
                        cached_fow.tile_visibility(t) != TileVisibility::Unexplored
                    })
                });
                let pos = Vec3::new((wa[0] + wb[0]) * 0.5, by, (wa[2] + wb[2]) * 0.5);
                push(
                    BridgeSpanKey::IntactSlice(ax, p, l0, l1),
                    false,
                    pos,
                    rot,
                    Vec3::new(sw, 1.0, sl),
                    vis,
                );
            }
            p += step;
        }
    }

    for (idx, sp) in world.destroyed_bridges.iter().enumerate() {
        let fp = if sp.rotated {
            [sp.footprint[1], sp.footprint[0]]
        } else {
            sp.footprint
        };
        let cw = world.grid.footprint_center_world(sp.tile_origin, fp);
        let vis = cached_fow.world_pos_visibility(world, cw) != TileVisibility::Unexplored;
        let wt = [sp.tile_origin[0] - 1, sp.tile_origin[1]];
        let nt = [sp.tile_origin[0], sp.tile_origin[1] - 1];
        let ax = if fp[0] != fp[1] {
            fp[0] > fp[1]
        } else {
            is_land(wt)
                || world.grid.bridge_tiles.contains(&wt)
                || !(is_land(nt) || world.grid.bridge_tiles.contains(&nt))
        };
        let (lt, wi) = if ax { (fp[0], fp[1]) } else { (fp[1], fp[0]) };
        let (len_m, sw) = (
            lt.max(1) as f32 * ts,
            ((wi.max(1) as f32 * ts) * 0.5).clamp(0.65, 2.4),
        );
        if len_m <= 2.6 {
            let rot = Quat::from_rotation_y(if ax { FRAC_PI_2 } else { 0.0 });
            let sc = Vec3::new(sw, 1.0, (len_m * 0.5).clamp(0.6, 1.3));
            push(
                BridgeSpanKey::DestroyedStub(idx, 0),
                true,
                Vec3::new(cw[0], by, cw[2]),
                rot,
                sc,
                vis,
            );
        } else {
            let hg = (len_m * 0.5 - 1.0).max(0.8);
            for (slot, sgn) in [(0u8, -1.0_f32), (1u8, 1.0_f32)] {
                let (pos, yaw) = if ax {
                    (Vec3::new(cw[0] + sgn * hg, by, cw[2]), -sgn * FRAC_PI_2)
                } else {
                    (
                        Vec3::new(cw[0], by, cw[2] + sgn * hg),
                        if sgn < 0.0 { 0.0 } else { PI },
                    )
                };
                let rot = Quat::from_rotation_y(yaw);
                push(
                    BridgeSpanKey::DestroyedStub(idx, slot),
                    true,
                    pos,
                    rot,
                    Vec3::new(sw, 1.0, 1.0),
                    vis,
                );
            }
        }
    }
    out
}

fn collect_bombed_entities(world: &SimWorld) -> HashMap<u64, f32> {
    let mut out: HashMap<u64, f32> = world
        .timed_bombs
        .iter()
        .filter(|(_, b)| b.remaining_secs > 0.0)
        .map(|(&eid, b)| (eid, b.remaining_secs))
        .collect();
    for (&eid, d) in &world.trait_stores.demolition {
        if d.planted_c4_owner.is_some() || d.pending_airstrike_timer > 0.0 {
            out.entry(eid).or_insert(d.pending_airstrike_timer.max(2.5));
        }
    }
    for (&eid, ent) in &world.entities {
        if let Some(b) = ent.active_buffs.iter().find(|b| {
            b.remaining_secs > 0.0
                && (b.delayed_burst_damage > 0.0 || b.id.contains("bomb") || b.id.contains("c4"))
        }) {
            out.entry(eid).or_insert(b.remaining_secs);
        }
    }
    out
}

#[derive(SystemParam)]
pub struct BridgeCrateSyncRes<'w> {
    pub time: Res<'w, Time>,
    pub asset_server: Option<Res<'w, AssetServer>>,
    pub asset_root: Option<Res<'w, AssetRootResource>>,
    pub mod_res: Option<Res<'w, ModResource>>,
    pub sim_res: Option<Res<'w, SimWorldResource>>,
    pub visual_mode: Option<Res<'w, Ra2EntityVisualMode>>,
    pub cached_fow: Option<Res<'w, CachedFogVisibility>>,
    pub smooth_fow: Option<Res<'w, SmoothFogTemporalState>>,
    pub fow_img_res: Option<Res<'w, FogOfWarOverlayImage>>,
    pub micro_res: Option<Res<'w, MicroDetailTextureResource>>,
    pub cache: ResMut<'w, BridgeAndCrateCache>,
    pub meshes: Option<ResMut<'w, Assets<Mesh>>>,
    pub std_mats: Option<ResMut<'w, Assets<StandardMaterial>>>,
    pub unit_pbr_mats: Option<ResMut<'w, Assets<UnitPbrTeamMaterial>>>,
}

type BridgeVis = BridgeSpan3dVisual;
type CrateVis = SupplyCrate3dVisual;
type BombVis = TimedBombBeacon3dVisual;
type PropVis = PropSceneVisualEntity;
type NodeInspect<'a> = (
    Option<&'a Name>,
    Option<&'a Mesh3d>,
    Option<&'a MeshMaterial3d<StandardMaterial>>,
);
type BombFilter = (
    Without<BridgeVis>,
    Without<CrateVis>,
    Without<PropVis>,
    Without<PreviewEnt>,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_3d_bridges_and_crates(
    mut commands: Commands,
    mut res: BridgeCrateSyncRes,
    cam_q: Query<&RtsCameraRig, Without<PreviewStageCamera>>,
    mut bridge_q: Query<
        (Entity, &mut BridgeVis, &mut Transform, &mut Visibility),
        (Without<CrateVis>, Without<PreviewEnt>),
    >,
    mut crate_q: Query<
        (Entity, &mut CrateVis, &mut Transform, &mut Visibility),
        (Without<BridgeVis>, Without<PreviewEnt>),
    >,
    mut prop_q: Query<
        (&PropVis, &mut Visibility),
        (Without<BridgeVis>, Without<CrateVis>, Without<PreviewEnt>),
    >,
    mut bomb_q: Query<(Entity, &BombVis, &mut Transform, &mut Visibility), BombFilter>,
    children_q: Query<&Children>,
    node_q: Query<NodeInspect, Without<PreviewEnt>>,
) {
    let (Some(mod_r), Some(sim_r), Some(cached_fow), Some(srv)) = (
        res.mod_res.as_deref(),
        res.sim_res.as_deref(),
        res.cached_fow.as_deref(),
        res.asset_server.as_deref(),
    ) else {
        return;
    };
    let (world, loaded, elapsed) = (&sim_r.0, &mod_r.0, res.time.elapsed_secs());
    let mode = res.visual_mode.as_deref().copied().unwrap_or_default();
    let is_3d = mode == Ra2EntityVisualMode::Full3dMeshes;
    LIVE_BRIDGE_VISUAL_MODE.store(if is_3d { 1 } else { 2 }, Ordering::Relaxed);

    if let (Some(ma), Some(sma)) = (res.meshes.as_deref_mut(), res.std_mats.as_deref_mut()) {
        res.cache.ensure_primitives(ma, sma);
    }
    if res.cache.loaded_mod_root.as_deref() != Some(loaded.root_dir.as_path()) {
        let ar = res.asset_root.as_deref();
        let ld = |rel: &str| {
            loaded.asset_exists(rel).then(|| {
                let p = ar.map(|a| resolve_mod_asset_path(&a.0, &loaded.root_dir, rel));
                srv.load(p.unwrap_or_else(|| format!("{rel}#Scene0")))
            })
        };
        res.cache.intact_bridge_scene = ld(BRIDGE_INTACT_REL_PATH);
        res.cache.destroyed_bridge_scene = ld(BRIDGE_DESTROYED_REL_PATH);
        res.cache.supply_crate_scene = ld(SUPPLY_CRATE_REL_PATH);
        res.cache.loaded_mod_root = Some(loaded.root_dir.clone());
    }

    let strategic_fade = 1.0
        - cam_q
            .iter()
            .next()
            .map_or(0.0, |r| r.strategic_zoom_blend());
    let def_fow = if cached_fow.fog_enabled { 1.0 } else { 0.0 };
    let fow_tex = res
        .fow_img_res
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_default();
    let micro_tex = res
        .micro_res
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_default();
    let fow_strength = res
        .smooth_fow
        .as_deref()
        .map_or(def_fow, |s| s.fow_shader_strength);
    let ts_grid = world.grid.tile_size;
    let world_size = Vec2::new(
        (world.grid.width.max(1) as f32 * ts_grid).max(1.0),
        (world.grid.height.max(1) as f32 * ts_grid).max(1.0),
    );
    let mut pbr_ctx = ScenePbrUpgradeCtx {
        std_mats: res.std_mats.as_deref(),
        ext_mats: res.unit_pbr_mats.as_deref_mut(),
        fow_tex,
        micro_tex,
        world_size,
        fow_strength,
        strategic_fade,
        elapsed,
    };
    let mut neutral_vdef = VisualDef::default();
    (neutral_vdef.team_color_tint, neutral_vdef.team_mask_mode) = (false, TeamMaskMode::Disabled);
    let mut crate_vdef = neutral_vdef.clone();
    crate_vdef.emissive_boost = Some(1.32);

    if should_use_3d_gltf_bridges(loaded, mode) {
        let mut desired: HashMap<BridgeSpanKey, DesiredBridgePlacement> =
            compute_desired_bridge_placements(world, cached_fow)
                .into_iter()
                .map(|p| (p.key, p))
                .collect();
        for (ent, mut sv, mut tf, mut vis) in &mut bridge_q {
            let Some(t) = desired.remove(&sv.key) else {
                commands.entity(ent).despawn_recursive();
                continue;
            };
            if sv.is_destroyed != t.is_destroyed {
                commands.entity(ent).despawn_recursive();
                desired.insert(t.key, t);
                continue;
            }
            (tf.translation, tf.rotation, tf.scale) = (t.position, t.rotation, t.scale);
            *vis = vis_if(t.visible && strategic_fade > 0.01);
            if !sv.meshes_upgraded {
                let tr = if t.is_destroyed {
                    DamageVisualTier::Yellow50
                } else {
                    DamageVisualTier::Intact
                };
                let sh = ShieldFresnelKind::None;
                let h_opt = upgrade_scene_hierarchy_to_unit_pbr(
                    &mut commands,
                    ent,
                    false,
                    &children_q,
                    &node_q,
                    &mut pbr_ctx,
                    &neutral_vdef,
                    [0.78, 0.80, 0.84, 1.0],
                    tr,
                    sh,
                );
                if let Some(h) = h_opt {
                    (sv.material_handles, sv.meshes_upgraded) = (h, true);
                }
            } else {
                pbr_ctx.refresh_handles(&sv.material_handles);
            }
        }
        for (key, t) in desired {
            let ih = res.cache.intact_bridge_scene.clone();
            let sc_opt = if t.is_destroyed {
                res.cache.destroyed_bridge_scene.clone().or(ih)
            } else {
                ih
            };
            if let Some(sh) = sc_opt {
                let comp = BridgeVis {
                    key,
                    is_destroyed: t.is_destroyed,
                    meshes_upgraded: false,
                    material_handles: Vec::new(),
                };
                let mut tf = Transform::from_translation(t.position).with_rotation(t.rotation);
                tf.scale = t.scale;
                let b = (MapEnv, comp, SceneRoot(sh));
                commands.spawn((b, vis_if(t.visible), tf));
            }
        }
    } else {
        for (ent, _, _, _) in &bridge_q {
            commands.entity(ent).despawn_recursive();
        }
    }

    if is_3d && res.cache.supply_crate_scene.is_some() {
        for (pv, mut vis) in &mut prop_q {
            if matches!(pv.prop_key, PropInstanceKey::Crate(_)) && *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        let mut cmap: HashMap<u64, (&rts_simulation::world::SimCrate, bool)> = world
            .crates
            .iter()
            .map(|c| {
                (
                    c.id,
                    (c, cached_fow.is_world_pos_visible(world, c.position)),
                )
            })
            .collect();
        for (ent, mut cv, mut tf, mut vis) in &mut crate_q {
            let Some((ci, is_vis)) = cmap.remove(&cv.crate_id) else {
                commands.entity(ent).despawn_recursive();
                continue;
            };
            let bob = 0.06 + (elapsed * 2.6 + ci.id as f32).sin() * 0.06;
            tf.translation = Vec3::new(ci.position[0], ci.position[1] + bob, ci.position[2]);
            tf.rotation = Quat::from_rotation_y(elapsed * 0.95 + ci.id as f32);
            tf.scale = Vec3::splat(1.0 + (elapsed * 3.4 + ci.id as f32).sin() * 0.035);
            *vis = vis_if(is_vis && strategic_fade > 0.01);
            if !cv.meshes_upgraded {
                let (tr, sh) = (DamageVisualTier::Intact, ShieldFresnelKind::None);
                let h_opt = upgrade_scene_hierarchy_to_unit_pbr(
                    &mut commands,
                    ent,
                    true,
                    &children_q,
                    &node_q,
                    &mut pbr_ctx,
                    &crate_vdef,
                    [0.85, 0.85, 0.85, 1.0],
                    tr,
                    sh,
                );
                if let Some(h) = h_opt {
                    (cv.material_handles, cv.meshes_upgraded) = (h, true);
                }
            } else {
                pbr_ctx.refresh_handles(&cv.material_handles);
            }
        }
        if let Some(ref cs) = res.cache.supply_crate_scene {
            for (crate_id, (ci, is_vis)) in cmap {
                let is_credits = matches!(ci.kind, CrateKind::BonusCredits);
                let (gr, cr) = (res.cache.gold_ring.clone(), res.cache.cyan_ring.clone());
                let (rmat, rm) = (
                    if is_credits { gr } else { cr },
                    res.cache.ring_mesh.clone(),
                );
                let mut comp = CrateVis::default();
                (comp.crate_id, comp.is_credits) = (crate_id, is_credits);
                let b = (MapEnv, comp, SceneRoot(cs.clone()));
                let tf = Transform::from_xyz(ci.position[0], ci.position[1], ci.position[2]);
                commands.spawn((b, vis_if(is_vis), tf)).with_children(|p| {
                    if let (Some(m), Some(mat)) = (rm, rmat) {
                        let rtf = Transform::from_xyz(0.0, 0.03, 0.0)
                            .with_scale(Vec3::new(1.15, 0.25, 1.15));
                        let rb = (SupplyCratePulseRing, Mesh3d(m), MeshMaterial3d(mat));
                        p.spawn((rb, NotShadowCaster, rtf));
                    }
                });
            }
        }
    } else {
        for (ent, _, _, _) in &crate_q {
            commands.entity(ent).despawn_recursive();
        }
    }

    let mut bombed = collect_bombed_entities(world);
    let b_pos = |se: &rts_simulation::world::SimEntity| -> Vec3 {
        let lift = if matches!(se.kind, EntityKind::Building { .. }) {
            3.95
        } else {
            2.35
        };
        Vec3::new(
            se.position[0],
            se.position[1].max(0.05) + lift,
            se.position[2],
        )
    };
    for (ent, beacon, mut tf, mut vis) in &mut bomb_q {
        let tid = beacon.target_sim_id;
        let Some((rem, Some(se))) = bombed.remove(&tid).map(|r| (r, world.entities.get(&tid)))
        else {
            commands.entity(ent).despawn_recursive();
            continue;
        };
        let freq = (4.5 + (4.5 - rem.clamp(0.0, 4.5)) * 2.4).clamp(4.5, 16.0);
        (tf.translation, tf.rotation) = (b_pos(se), Quat::from_rotation_y(elapsed * 2.2));
        tf.scale = Vec3::splat(0.65 + (elapsed * freq).sin().abs() * 0.75);
        *vis = vis_if(cached_fow.is_entity_visible(world, loaded, se));
    }
    if let (Some(bm), Some(sm), Some(bmat), Some(smat)) = (
        res.cache.c4_box.clone(),
        res.cache.c4_bulb.clone(),
        res.cache.c4_mat.clone(),
        res.cache.strobe_mat.clone(),
    ) {
        for eid in bombed.into_keys() {
            if let Some(se) = world.entities.get(&eid) {
                let b = (MapEnv, BombVis { target_sim_id: eid }, Mesh3d(bm.clone()));
                commands
                    .spawn((
                        b,
                        MeshMaterial3d(bmat.clone()),
                        Transform::from_translation(b_pos(se)),
                    ))
                    .with_children(|p| {
                        let sb = (
                            Mesh3d(sm.clone()),
                            MeshMaterial3d(smat.clone()),
                            NotShadowCaster,
                        );
                        p.spawn((sb, Transform::from_xyz(0.0, 0.16, 0.0)));
                    });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_simulation::world::DestroyedBridgeSpan;

    #[test]
    fn bridge_axis_resolver_orients_ew_and_ns_spans_and_demolished_stubs_cleanly() {
        let loaded = LoadedMod::load_from_dir(LoadedMod::resolve_mod_dir("rust-ra2")).unwrap();
        let mut world = SimWorld::from_mod_map(&loaded, &loaded.entry_map);
        world.grid = CollisionGrid::new(32, 32, 2.0);
        for z in 0..32 {
            for x in 14..=17 {
                world.grid.water_tiles.insert([x, z]);
            }
        }
        for x in 14..=17 {
            for z in 10..=11 {
                world.grid.bridge_tiles.insert([x, z]);
            }
        }
        world.destroyed_bridges.push(DestroyedBridgeSpan {
            building_id: "bridge".to_string(),
            tile_origin: [14, 20],
            footprint: [4, 2],
            rotated: false,
            destroyed_by_player_id: 2,
        });
        let pls = compute_desired_bridge_placements(&world, &CachedFogVisibility::default());
        assert_eq!(pls.iter().filter(|p| !p.is_destroyed).count(), 4);
        assert_eq!(pls.iter().filter(|p| p.is_destroyed).count(), 2);
        for p in pls.iter().filter(|p| !p.is_destroyed) {
            let (yaw, _, _) = p.rotation.to_euler(EulerRot::YXZ);
            assert!((yaw.abs() - FRAC_PI_2).abs() < 1e-3);
            assert!((p.position.y - CollisionGrid::WATER_SURFACE_Y).abs() < 1e-4);
        }
    }
}
