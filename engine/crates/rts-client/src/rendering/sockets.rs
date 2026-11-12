use super::preview_stage::PreviewStageEntity;
use super::SimVisualEntity;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use rts_simulation::world::EntityKind;
use std::collections::HashMap;

pub const KNOWN_SOCKET_NAMES: [&str; 6] = [
    "Muzzle_01",
    "Muzzle_02",
    "Exhaust_01",
    "Exhaust",
    "DamageSmoke_01",
    "DamageFire",
];

pub const MAX_ENTITY_MUZZLES: usize = 4;
pub const MAX_NAMED_SOCKETS: usize = 8;
pub const INLINE_SOCKET_NAME_CAP: usize = 24;

/// Stack-allocated UTF-8 socket identifier (`<= 24` bytes, zero heap allocations).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InlineSocketName {
    bytes: [u8; INLINE_SOCKET_NAME_CAP],
    len: u8,
}

impl Default for InlineSocketName {
    fn default() -> Self {
        Self {
            bytes: [0u8; INLINE_SOCKET_NAME_CAP],
            len: 0,
        }
    }
}

impl std::fmt::Debug for InlineSocketName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl InlineSocketName {
    #[inline]
    pub fn from_str_trunc(s: &str) -> Self {
        let raw = s.as_bytes();
        let mut cap = raw.len().min(INLINE_SOCKET_NAME_CAP);
        while cap > 0 && !s.is_char_boundary(cap) {
            cap -= 1;
        }
        let mut bytes = [0u8; INLINE_SOCKET_NAME_CAP];
        bytes[..cap].copy_from_slice(&raw[..cap]);
        Self {
            bytes,
            len: cap as u8,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn starts_with_ignore_ascii_case(&self, prefix: &str) -> bool {
        let p = prefix.as_bytes();
        if (self.len as usize) < p.len() {
            return false;
        }
        self.bytes[..p.len()].eq_ignore_ascii_case(p)
    }
}

/// Stack-allocated glTF child socket hierarchy cached once on entity spawn (`0` heap allocations).
#[derive(Debug, Clone, Copy)]
pub struct DiscoveredSocketHierarchy {
    pub socket_nodes: [(InlineSocketName, Entity); MAX_NAMED_SOCKETS],
    pub socket_node_len: u8,
    pub turret_node: Option<(Entity, Vec3)>,
    pub barrel_node: Option<(Entity, Vec3)>,
    pub rotor_node: Option<(Entity, Vec3)>,
}

impl Default for DiscoveredSocketHierarchy {
    fn default() -> Self {
        Self {
            socket_nodes: [(InlineSocketName::default(), Entity::PLACEHOLDER); MAX_NAMED_SOCKETS],
            socket_node_len: 0,
            turret_node: None,
            barrel_node: None,
            rotor_node: None,
        }
    }
}

impl DiscoveredSocketHierarchy {
    #[inline]
    pub fn push_socket_node(&mut self, name: &str, entity: Entity) {
        let idx = self.socket_node_len as usize;
        if idx < MAX_NAMED_SOCKETS {
            self.socket_nodes[idx] = (InlineSocketName::from_str_trunc(name), entity);
            self.socket_node_len += 1;
        }
    }

    #[inline]
    pub fn iter_socket_nodes(&self) -> impl Iterator<Item = (&str, Entity)> {
        self.socket_nodes[..self.socket_node_len as usize]
            .iter()
            .map(|(n, e)| (n.as_str(), *e))
    }
}

/// Zero-heap-allocation fixed-capacity socket table per active simulation entity (`Task 205b`).
#[derive(Debug, Clone, Copy)]
pub struct FixedEntitySockets {
    pub muzzles: [Vec3; MAX_ENTITY_MUZZLES],
    pub muzzle_len: u8,
    pub muzzle_cursor: u8,
    pub exhaust: Option<Vec3>,
    pub damage_smoke: Option<Vec3>,
    pub turret_offset: Option<Vec3>,
    pub barrel_offset: Option<Vec3>,
    pub rotor_offset: Option<Vec3>,
    pub named_slots: [(InlineSocketName, Vec3); MAX_NAMED_SOCKETS],
    pub named_len: u8,
    pub initialized_once: bool,
    last_seen_epoch: u32,
    cached_nodes: Option<DiscoveredSocketHierarchy>,
}

impl Default for FixedEntitySockets {
    fn default() -> Self {
        Self {
            muzzles: [Vec3::ZERO; MAX_ENTITY_MUZZLES],
            muzzle_len: 0,
            muzzle_cursor: 0,
            exhaust: None,
            damage_smoke: None,
            turret_offset: None,
            barrel_offset: None,
            rotor_offset: None,
            named_slots: [(InlineSocketName::default(), Vec3::ZERO); MAX_NAMED_SOCKETS],
            named_len: 0,
            initialized_once: false,
            last_seen_epoch: 0,
            cached_nodes: None,
        }
    }
}

/// Backwards-compatible type alias for `FixedEntitySockets`.
pub type EntitySocketState = FixedEntitySockets;

impl FixedEntitySockets {
    pub fn from_muzzles(muzzles: &[Vec3]) -> Self {
        let mut out = Self::default();
        for &m in muzzles.iter().take(MAX_ENTITY_MUZZLES) {
            out.push_muzzle(m);
        }
        out
    }

    #[inline]
    pub fn clear_world_positions(&mut self) {
        self.muzzle_len = 0;
        self.exhaust = None;
        self.damage_smoke = None;
        self.named_len = 0;
    }

    #[inline]
    pub fn push_muzzle(&mut self, pos: Vec3) {
        let idx = self.muzzle_len as usize;
        if idx < MAX_ENTITY_MUZZLES {
            self.muzzles[idx] = pos;
            self.muzzle_len += 1;
        }
    }

    #[inline]
    pub fn muzzle_slice(&self) -> &[Vec3] {
        &self.muzzles[..self.muzzle_len as usize]
    }

    #[inline]
    pub fn upsert_named(&mut self, name: &str, pos: Vec3) {
        for slot in &mut self.named_slots[..self.named_len as usize] {
            if slot.0.as_str().eq_ignore_ascii_case(name) {
                slot.0 = InlineSocketName::from_str_trunc(name);
                slot.1 = pos;
                return;
            }
        }
        let idx = self.named_len as usize;
        if idx < MAX_NAMED_SOCKETS {
            self.named_slots[idx] = (InlineSocketName::from_str_trunc(name), pos);
            self.named_len += 1;
        }
    }

    #[inline]
    pub fn get_socket(&self, name: &str) -> Option<Vec3> {
        self.named_slots[..self.named_len as usize]
            .iter()
            .find(|(n, _)| n.as_str().eq_ignore_ascii_case(name))
            .map(|(_, pos)| *pos)
    }

    #[inline]
    pub fn iter_named(&self) -> impl Iterator<Item = (&str, Vec3)> {
        self.named_slots[..self.named_len as usize]
            .iter()
            .map(|(n, pos)| (n.as_str(), *pos))
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SocketRegistry {
    pub entries: HashMap<u64, FixedEntitySockets>,
    frame_epoch: u32,
}

impl SocketRegistry {
    /// Returns the next world-space muzzle origin for `sim_id`, alternating across `Muzzle_01` / `Muzzle_02`
    /// for dual-barrel tanks, AA guns, aircraft pods, and dual-wield infantry.
    pub fn next_muzzle_pos(&mut self, sim_id: u64) -> Option<Vec3> {
        let entry = self.entries.get_mut(&sim_id)?;
        let len = entry.muzzle_len as usize;
        if len == 0 {
            return None;
        }
        let idx = (entry.muzzle_cursor as usize) % len;
        entry.muzzle_cursor = entry.muzzle_cursor.wrapping_add(1);
        Some(entry.muzzles[idx])
    }

    pub fn primary_muzzle_pos(&self, sim_id: u64) -> Option<Vec3> {
        self.entries
            .get(&sim_id)
            .and_then(|e| e.muzzle_slice().first().copied())
    }

    pub fn exhaust_pos(&self, sim_id: u64) -> Option<Vec3> {
        self.entries.get(&sim_id).and_then(|e| e.exhaust)
    }

    pub fn damage_smoke_pos(&self, sim_id: u64) -> Option<Vec3> {
        self.entries.get(&sim_id).and_then(|e| e.damage_smoke)
    }

    pub fn get_socket(&self, sim_id: u64, socket_name: &str) -> Option<Vec3> {
        self.entries
            .get(&sim_id)
            .and_then(|e| e.get_socket(socket_name))
    }

    pub fn iter_all_sockets(&self) -> impl Iterator<Item = (u64, &str, Vec3)> {
        self.entries
            .iter()
            .flat_map(|(&id, state)| state.iter_named().map(move |(name, pos)| (id, name, pos)))
    }
}

/// Pure world-space fallback socket transform helper: rotates `Muzzle*` offsets by turret yaw and
/// chassis/structure sockets (`Exhaust*`, `DamageSmoke*`) by hull yaw when no child `Turret` node is loaded.
pub fn transform_local_socket_to_world(
    entity_world_pos: Vec3,
    chassis_yaw: f32,
    turret_yaw: f32,
    scale: Vec3,
    socket_name: &str,
    local_offset: [f32; 3],
) -> Vec3 {
    let scaled = Vec3::from_array(local_offset) * scale;
    let is_turret_bound = socket_name
        .as_bytes()
        .get(..6)
        .is_some_and(|p| p.eq_ignore_ascii_case(b"muzzle"));
    let yaw = if is_turret_bound {
        turret_yaw
    } else {
        chassis_yaw
    };
    entity_world_pos + Quat::from_rotation_y(yaw) * scaled
}

/// Transforms a turret/barrel-bound socket offset through the live 3D `GlobalTransform` of the
/// `Turret` or `Barrel` child node so turret yaw, elevation pitch, recoil kickback, and hull tilt
/// are inherited with zero drift.
pub fn transform_turret_socket_via_global(
    turret_global: &GlobalTransform,
    turret_rest_local: Vec3,
    socket_offset: [f32; 3],
) -> Vec3 {
    let rel = Vec3::from_array(socket_offset) - turret_rest_local;
    turret_global.transform_point(rel)
}

#[inline]
fn global_transform_is_valid(gtf: &GlobalTransform) -> bool {
    let aff = gtf.affine();
    aff.translation.is_finite() && aff.matrix3.col(0).length_squared() > 1e-6
}

#[inline]
fn starts_with_ignore_ascii_case(s: &str, prefix: &[u8]) -> bool {
    s.as_bytes()
        .get(..prefix.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
}

#[allow(clippy::type_complexity)]
fn discover_socket_hierarchy(
    root: Entity,
    children_q: &Query<&Children>,
    node_q: &Query<(&Name, &Transform, &GlobalTransform), Without<PreviewStageEntity>>,
) -> Option<DiscoveredSocketHierarchy> {
    let root_children = children_q.get(root).ok()?;
    if root_children.is_empty() {
        return None;
    }
    let mut any_named = false;
    let mut hier = DiscoveredSocketHierarchy::default();
    let mut stack = [(Entity::PLACEHOLDER, Vec3::ZERO); 48];
    let mut stack_len = 1usize;
    stack[0] = (root, Vec3::ZERO);

    while stack_len > 0 {
        stack_len -= 1;
        let (curr, parent_accum_pos) = stack[stack_len];
        if let Ok(children) = children_q.get(curr) {
            for &child in children.iter() {
                let mut child_accum = parent_accum_pos;
                if let Ok((name, local_tf, _)) = node_q.get(child) {
                    any_named = true;
                    let s = name.as_str();
                    if s.ends_with("_LOD1") {
                        continue;
                    }
                    child_accum = parent_accum_pos + local_tf.translation;
                    if s == "Turret" && hier.turret_node.is_none() {
                        hier.turret_node = Some((child, child_accum));
                    } else if s == "Barrel" && hier.barrel_node.is_none() {
                        hier.barrel_node = Some((child, child_accum));
                    } else if s == "Rotor" && hier.rotor_node.is_none() {
                        hier.rotor_node = Some((child, child_accum));
                    }
                    if KNOWN_SOCKET_NAMES.contains(&s)
                        || starts_with_ignore_ascii_case(s, b"muzzle")
                        || starts_with_ignore_ascii_case(s, b"exhaust")
                        || starts_with_ignore_ascii_case(s, b"damage")
                    {
                        hier.push_socket_node(s, child);
                    }
                }
                if stack_len < stack.len() {
                    stack[stack_len] = (child, child_accum);
                    stack_len += 1;
                }
            }
        }
    }
    if any_named {
        Some(hier)
    } else {
        None
    }
}

#[allow(clippy::type_complexity)]
pub fn sync_socket_registry(
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    mut registry: ResMut<SocketRegistry>,
    visual_q: Query<
        (
            Entity,
            &SimVisualEntity,
            &Transform,
            &GlobalTransform,
            Option<&Visibility>,
        ),
        Without<PreviewStageEntity>,
    >,
    children_q: Query<&Children>,
    node_q: Query<(&Name, &Transform, &GlobalTransform), Without<PreviewStageEntity>>,
) {
    let (Some(mod_res), Some(sim_res)) = (mod_res, sim_res) else {
        return;
    };
    let world = &sim_res.0;
    let loaded = &mod_res.0;

    registry.frame_epoch = registry.frame_epoch.wrapping_add(1);
    if registry.frame_epoch == 0 {
        registry.frame_epoch = 1;
    }
    let epoch = registry.frame_epoch;

    for (bevy_ent, vis, root_tf, root_gtf, vis_opt) in &visual_q {
        let Some(sim_ent) = world.entities.get(&vis.sim_id) else {
            continue;
        };

        let entry = registry.entries.entry(vis.sim_id).or_default();
        entry.last_seen_epoch = epoch;

        // Skip per-frame socket recalculation for shrouded/hidden entities once initialized
        if vis_opt.copied() == Some(Visibility::Hidden) && entry.initialized_once {
            continue;
        }

        if entry.cached_nodes.is_none() {
            if let Some(hier) = discover_socket_hierarchy(bevy_ent, &children_q, &node_q) {
                entry.turret_offset = hier.turret_node.map(|(_, off)| off);
                entry.barrel_offset = hier.barrel_node.map(|(_, off)| off);
                entry.rotor_offset = hier.rotor_node.map(|(_, off)| off);
                entry.cached_nodes = Some(hier);
            }
        }

        entry.clear_world_positions();

        let mut live_turret_gtf: Option<(GlobalTransform, Vec3)> = None;
        let mut live_barrel_gtf: Option<(GlobalTransform, Vec3)> = None;

        // 1. Read live glTF child node GlobalTransforms first (including Turret & Barrel hierarchy)
        if let Some(hier) = entry.cached_nodes {
            if let Some((t_ent, t_rest)) = hier.turret_node {
                if let Ok((_, _, gtf)) = node_q.get(t_ent) {
                    if global_transform_is_valid(gtf) {
                        live_turret_gtf = Some((*gtf, t_rest));
                    }
                }
            }
            if let Some((b_ent, b_rest)) = hier.barrel_node {
                if let Ok((_, _, gtf)) = node_q.get(b_ent) {
                    if global_transform_is_valid(gtf) {
                        live_barrel_gtf = Some((*gtf, b_rest));
                    }
                }
            }
            for (sname, n_ent) in hier.iter_socket_nodes() {
                if let Ok((_, _, gtf)) = node_q.get(n_ent) {
                    let wpos = gtf.translation();
                    if wpos.is_finite() && wpos.length_squared() > 1e-6 {
                        entry.upsert_named(sname, wpos);
                    }
                }
            }
        }

        // 2. Apply explicit `VisualDef.sockets` overrides attached to child `Barrel`/`Turret` `GlobalTransform`
        let visual_def = match &sim_ent.kind {
            EntityKind::Unit { unit_id } => loaded.units.get(unit_id).map(|u| &u.visual),
            EntityKind::Building { building_id } => {
                loaded.buildings.get(building_id).map(|b| &b.visual)
            }
        };
        let shp_iso_yaw = if vis.building_footprint.is_some() {
            38.0_f32.to_radians()
        } else {
            0.0
        };
        let chassis_yaw = sim_ent.facing_radians + shp_iso_yaw;
        let turret_yaw = sim_ent.turret_facing_radians;
        let active_gun_mount = live_barrel_gtf.or(live_turret_gtf);

        let compute_socket_world =
            |sock_name: &str, offset: [f32; 3], scale_override: Vec3| -> Vec3 {
                let is_muzzle = starts_with_ignore_ascii_case(sock_name, b"muzzle");
                if is_muzzle {
                    if let Some((ref mount_gtf, mount_rest)) = active_gun_mount {
                        return transform_turret_socket_via_global(mount_gtf, mount_rest, offset);
                    }
                } else if global_transform_is_valid(root_gtf) {
                    return root_gtf.transform_point(Vec3::from_array(offset));
                }
                transform_local_socket_to_world(
                    root_tf.translation,
                    chassis_yaw,
                    turret_yaw,
                    scale_override,
                    sock_name,
                    offset,
                )
            };

        if let Some(vdef) = visual_def {
            for sock in &vdef.sockets {
                let wpos = compute_socket_world(&sock.name, sock.offset, root_tf.scale);
                entry.upsert_named(&sock.name, wpos);
            }
        }

        // Collect ordered muzzles (`Muzzle_01`, `Muzzle_02`, ...) without heap allocations
        let named_count = entry.named_len as usize;
        for i in 0..named_count {
            let (sname, pos) = entry.named_slots[i];
            if sname.starts_with_ignore_ascii_case("muzzle") {
                entry.push_muzzle(pos);
            } else if sname.starts_with_ignore_ascii_case("exhaust") && entry.exhaust.is_none() {
                entry.exhaust = Some(pos);
            } else if sname.starts_with_ignore_ascii_case("damage") && entry.damage_smoke.is_none()
            {
                entry.damage_smoke = Some(pos);
            }
        }

        // 3. Deterministic fallback if neither glTF socket node nor VisualDef override is present yet
        if entry.muzzle_len == 0 {
            let fallback_local = if vis.building_footprint.is_some() {
                [0.0, 1.35, 1.35]
            } else {
                [0.0, 0.95, 1.25]
            };
            let mpos = compute_socket_world("Muzzle_01", fallback_local, Vec3::ONE);
            entry.push_muzzle(mpos);
            entry.upsert_named("Muzzle_01", mpos);
        }
        if entry.exhaust.is_none() && vis.building_footprint.is_none() {
            let epos = compute_socket_world("Exhaust_01", [0.0, 0.45, -1.05], Vec3::ONE);
            entry.exhaust = Some(epos);
            entry.upsert_named("Exhaust_01", epos);
        }
        if entry.damage_smoke.is_none() {
            let local = if vis.building_footprint.is_some() {
                [0.2, 1.75, -0.15]
            } else {
                [0.0, 1.05, -0.2]
            };
            let dpos = compute_socket_world("DamageSmoke_01", local, Vec3::ONE);
            entry.damage_smoke = Some(dpos);
            entry.upsert_named("DamageSmoke_01", dpos);
        }

        entry.initialized_once = true;
    }

    registry
        .entries
        .retain(|_, state| state.last_seen_epoch == epoch);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_world_transform_and_zero_alloc_fixed_sockets_are_exact() {
        // Verify FixedEntitySockets is `Copy` (pure stack value, zero heap pointers!)
        fn assert_copy<T: Copy>() {}
        assert_copy::<FixedEntitySockets>();

        let pos = Vec3::new(10.0, 1.5, 20.0);
        // Turret pointing +X (yaw = +FRAC_PI_2): local +Z barrel offset (0, 1.0, 2.0) maps to (+2.0, +1.0, 0.0)
        let muzzle_world = transform_local_socket_to_world(
            pos,
            0.0,
            std::f32::consts::FRAC_PI_2,
            Vec3::ONE,
            "Muzzle_01",
            [0.0, 1.0, 2.0],
        );
        assert!((muzzle_world - Vec3::new(12.0, 2.5, 20.0)).length() < 1e-4);

        // Exhaust uses chassis yaw (0.0), NOT turret yaw (+FRAC_PI_2)
        let exhaust_world = transform_local_socket_to_world(
            pos,
            0.0,
            std::f32::consts::FRAC_PI_2,
            Vec3::ONE,
            "Exhaust_01",
            [0.0, 0.5, -1.5],
        );
        assert!((exhaust_world - Vec3::new(10.0, 2.0, 18.5)).length() < 1e-4);

        // 3D Turret GlobalTransform with +30 deg barrel elevation pitch (-PI/6 around X) and recoil:
        let pitch_up_30 = -std::f32::consts::FRAC_PI_6;
        let turret_rest = Vec3::new(0.0, 1.0, 0.0);
        let turret_gtf = GlobalTransform::from(
            Transform::from_xyz(10.0, 2.5, 19.75).with_rotation(Quat::from_rotation_x(pitch_up_30)),
        );
        let pitched_muzzle =
            transform_turret_socket_via_global(&turret_gtf, turret_rest, [0.0, 1.0, 2.0]);
        assert!((pitched_muzzle.x - 10.0).abs() < 1e-4);
        assert!(
            (pitched_muzzle.y - 3.5).abs() < 1e-4,
            "Pitched turret socket must rise with barrel elevation"
        );
        assert!((pitched_muzzle.z - (19.75 + 2.0 * 30.0_f32.to_radians().cos())).abs() < 1e-4);

        let mut fixed = FixedEntitySockets::from_muzzles(&[
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        ]);
        fixed.upsert_named("Muzzle_01", Vec3::new(1.0, 1.0, 0.0));
        fixed.upsert_named("Muzzle_02", Vec3::new(-1.0, 1.0, 0.0));
        fixed.turret_offset = Some(Vec3::new(0.0, 0.85, 0.0));

        let mut reg = SocketRegistry::default();
        reg.entries.insert(42, fixed);
        assert_eq!(reg.next_muzzle_pos(42), Some(Vec3::new(1.0, 1.0, 0.0)));
        assert_eq!(reg.next_muzzle_pos(42), Some(Vec3::new(-1.0, 1.0, 0.0)));
        assert_eq!(reg.next_muzzle_pos(42), Some(Vec3::new(1.0, 1.0, 0.0)));
        assert_eq!(
            reg.get_socket(42, "muzzle_02"),
            Some(Vec3::new(-1.0, 1.0, 0.0))
        );
        assert_eq!(reg.iter_all_sockets().count(), 2);
    }
}
