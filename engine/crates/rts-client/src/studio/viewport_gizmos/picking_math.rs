use crate::input::ClientInteractionState;
use crate::studio::dock_layout::{focus_or_open_tab, StudioTab};
use crate::studio::history::{EntityTransformSnap, TerrainStrokeState};
use crate::studio::world_builder::WorldBuilderTool;
use crate::studio::{SdkInspectTarget, StudioState};
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::MovementDomain;
use rts_simulation::world::{EntityKind, SimWorld};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioGizmoHandle {
    MoveX,
    MoveY,
    MoveZ,
    RotateY,
    ZoneCorner { zone_idx: usize, corner: u8 },
}

#[derive(Debug, Clone)]
pub struct ActiveGizmoDrag {
    pub handle: StudioGizmoHandle,
    pub pivot: Vec3,
    pub start_axis_param: f32,
    pub start_angle: f32,
    pub entity_before: Vec<EntityTransformSnap>,
    pub map_before: Option<TerrainStrokeState>,
    pub mutated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickedViewportObject {
    SimEntity(u64),
    Prop(u64),
    CivilianStructure(u64),
    PlayerSpawn(u8),
    TriggerZone(usize),
}

/// Computes the closest scalar parameter `t` along the 3D axis line `pivot + axis_dir * t` to `ray`.
pub fn closest_ray_axis_param(ray: Ray3d, pivot: Vec3, axis_dir: Vec3) -> f32 {
    let ro = ray.origin;
    let rd = *ray.direction;
    let w0 = ro - pivot;
    let b = rd.dot(axis_dir);
    let d = rd.dot(w0);
    let e = axis_dir.dot(w0);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-4 {
        return e;
    }
    (b * d - e) / denom
}

/// Shortest world-space distance between `ray` and segment `[a, b]`.
pub fn ray_segment_distance(ray: Ray3d, a: Vec3, b: Vec3) -> f32 {
    let seg = b - a;
    let seg_len = seg.length();
    if seg_len < 1e-4 {
        return (*ray.direction).cross(a - ray.origin).length();
    }
    let dir = seg / seg_len;
    let s = closest_ray_axis_param(ray, a, dir).clamp(0.0, seg_len);
    let pt_seg = a + dir * s;
    let t_ray = (pt_seg - ray.origin).dot(*ray.direction).max(0.0);
    let pt_ray = ray.origin + *ray.direction * t_ray;
    pt_seg.distance(pt_ray)
}

/// Intersects `ray` with horizontal plane `y = plane_y`.
pub fn ray_horizontal_plane_hit(ray: Ray3d, plane_y: f32) -> Option<Vec3> {
    if ray.direction.y.abs() <= 1e-5 {
        return None;
    }
    let t = (plane_y - ray.origin.y) / ray.direction.y;
    (t > 0.0).then(|| ray.origin + *ray.direction * t)
}

/// Computes the active Studio 3D selection centroid across entities, props, civilian structures, spawns, and zones.
pub fn compute_studio_selection_centroid(
    state: &StudioState,
    interaction: &ClientInteractionState,
    world: &SimWorld,
    loaded: &LoadedMod,
) -> Option<Vec3> {
    let mut sum = Vec3::ZERO;
    let mut count = 0usize;
    let ts = world.grid.tile_size.max(1.0);

    for &eid in &interaction.selected_ids {
        if state.locked_entity_ids.contains(&eid) || state.hidden_entity_ids.contains(&eid) {
            continue;
        }
        if let Some(e) = world.entities.get(&eid) {
            let y = world
                .grid
                .sample_world_height(e.position[0], e.position[2])
                .max(e.position[1]);
            sum += Vec3::new(e.position[0], y, e.position[2]);
            count += 1;
        }
    }
    for &pid in &state.selected_prop_ids {
        if let Some(p) = world.props.iter().find(|p| p.id == pid) {
            let y = world.grid.sample_world_height(p.position[0], p.position[2]);
            sum += Vec3::new(p.position[0], y, p.position[2]);
            count += 1;
        }
    }
    for &cid in &state.selected_civ_ids {
        if let Some(c) = world.civilian_structures.iter().find(|c| c.id == cid) {
            let y = world.grid.sample_world_height(c.position[0], c.position[2]);
            sum += Vec3::new(c.position[0], y, c.position[2]);
            count += 1;
        }
    }
    for &sp_pid in &state.selected_spawn_players {
        if let Some(sp) = loaded
            .entry_map
            .spawns
            .iter()
            .find(|s| s.player_id == sp_pid)
        {
            let wx = (sp.base_tile[0] as f32 + 0.5) * ts;
            let wz = (sp.base_tile[1] as f32 + 0.5) * ts;
            let y = world.grid.sample_world_height(wx, wz);
            sum += Vec3::new(wx, y, wz);
            count += 1;
        }
    }
    for &z_idx in &state.selected_zone_indices {
        if let Some(z) = loaded.entry_map.trigger_zones.get(z_idx) {
            let wx = (z.min[0] + z.max[0] + 1) as f32 * 0.5 * ts;
            let wz = (z.min[1] + z.max[1] + 1) as f32 * 0.5 * ts;
            let y = world.grid.sample_world_height(wx, wz);
            sum += Vec3::new(wx, y, wz);
            count += 1;
        }
    }

    (count > 0).then(|| sum / count as f32)
}

pub(super) fn zone_corner_world(
    zone: &rts_sdk::schema::TriggerZoneDef,
    corner: u8,
    grid: &rts_simulation::pathfinding::CollisionGrid,
) -> Vec3 {
    let ts = grid.tile_size.max(1.0);
    let (tx, tz) = match corner {
        0 => (zone.min[0] as f32, zone.min[1] as f32),
        1 => ((zone.max[0] + 1) as f32, zone.min[1] as f32),
        2 => ((zone.max[0] + 1) as f32, (zone.max[1] + 1) as f32),
        _ => (zone.min[0] as f32, (zone.max[1] + 1) as f32),
    };
    let wx = tx * ts;
    let wz = tz * ts;
    Vec3::new(wx, grid.sample_world_height(wx, wz) + 0.35, wz)
}

pub(super) fn hit_test_gizmo_handles(
    ray: Ray3d,
    pivot_opt: Option<Vec3>,
    state: &StudioState,
    world: &SimWorld,
    loaded: &LoadedMod,
) -> Option<StudioGizmoHandle> {
    // 1. TriggerZone corner handles first when a zone is selected
    for &z_idx in &state.selected_zone_indices {
        if let Some(zone) = loaded.entry_map.trigger_zones.get(z_idx) {
            for corner in 0..4u8 {
                let cpos = zone_corner_world(zone, corner, &world.grid);
                let dist = (*ray.direction).cross(cpos - ray.origin).length();
                if dist <= 0.85 {
                    return Some(StudioGizmoHandle::ZoneCorner {
                        zone_idx: z_idx,
                        corner,
                    });
                }
            }
        }
    }

    let pivot = pivot_opt?;
    let base = pivot + Vec3::Y * 0.45;
    let axis_len = 3.2;
    let pick_tol = 0.55;

    let dx = ray_segment_distance(ray, base, base + Vec3::X * axis_len);
    let dy = ray_segment_distance(ray, base, base + Vec3::Y * axis_len);
    let dz = ray_segment_distance(ray, base, base + Vec3::Z * axis_len);
    let mut best = (pick_tol, None);
    if dx < best.0 {
        best = (dx, Some(StudioGizmoHandle::MoveX));
    }
    if dy < best.0 {
        best = (dy, Some(StudioGizmoHandle::MoveY));
    }
    if dz < best.0 {
        best = (dz, Some(StudioGizmoHandle::MoveZ));
    }
    if best.1.is_some() {
        return best.1;
    }

    // Y-rotation ring at radius `2.5` on `y = base.y`
    if let Some(hit) = ray_horizontal_plane_hit(ray, base.y) {
        let r = Vec2::new(hit.x - base.x, hit.z - base.z).length();
        if (r - 2.5).abs() <= 0.42 {
            return Some(StudioGizmoHandle::RotateY);
        }
    }
    None
}

/// Ray/ground-point pick across `SimEntity`, `SimProp`, `SimCivilianStructure`, `PlayerSpawnDef`, and `TriggerZoneDef`.
pub fn pick_studio_object_at_ray(
    ray: Ray3d,
    ground_hit: Vec3,
    state: &StudioState,
    world: &SimWorld,
    loaded: &LoadedMod,
) -> Option<PickedViewportObject> {
    let ts = world.grid.tile_size.max(1.0);
    let mut best_dist = 2.6f32;
    let mut best_pick = None;

    for ent in world.entities.values() {
        if ent.health <= 0.0
            || state.hidden_entity_ids.contains(&ent.id)
            || state.locked_entity_ids.contains(&ent.id)
        {
            continue;
        }
        let center = Vec3::new(
            ent.position[0],
            world
                .grid
                .sample_world_height(ent.position[0], ent.position[2])
                .max(ent.position[1])
                + 0.7,
            ent.position[2],
        );
        let ray_d = (*ray.direction).cross(center - ray.origin).length();
        let gnd_d = Vec2::new(
            ent.position[0] - ground_hit.x,
            ent.position[2] - ground_hit.z,
        )
        .length();
        let d = ray_d.min(gnd_d);
        let radius = match ent.kind {
            EntityKind::Building { .. } => 2.5,
            EntityKind::Unit { .. } => 1.6,
        };
        if d <= radius && d < best_dist {
            best_dist = d;
            best_pick = Some(PickedViewportObject::SimEntity(ent.id));
        }
    }

    for prop in &world.props {
        let gnd_d = Vec2::new(
            prop.position[0] - ground_hit.x,
            prop.position[2] - ground_hit.z,
        )
        .length();
        if gnd_d <= 1.35 && gnd_d < best_dist {
            best_dist = gnd_d;
            best_pick = Some(PickedViewportObject::Prop(prop.id));
        }
    }

    for civ in &world.civilian_structures {
        let gnd_d = Vec2::new(
            civ.position[0] - ground_hit.x,
            civ.position[2] - ground_hit.z,
        )
        .length();
        let rad = (civ.footprint[0].max(civ.footprint[1]) as f32) * 0.55 * ts;
        if gnd_d <= rad && gnd_d < best_dist {
            best_dist = gnd_d;
            best_pick = Some(PickedViewportObject::CivilianStructure(civ.id));
        }
    }

    for sp in &loaded.entry_map.spawns {
        let wx = (sp.base_tile[0] as f32 + 0.5) * ts;
        let wz = (sp.base_tile[1] as f32 + 0.5) * ts;
        let gnd_d = Vec2::new(wx - ground_hit.x, wz - ground_hit.z).length();
        if gnd_d <= 2.0 && gnd_d < best_dist {
            best_dist = gnd_d;
            best_pick = Some(PickedViewportObject::PlayerSpawn(sp.player_id));
        }
    }

    if best_pick.is_some() {
        return best_pick;
    }

    // Check TriggerZone interiors
    let hit_tile = world.grid.world_to_tile([ground_hit.x, 0.0, ground_hit.z]);
    for (idx, z) in loaded.entry_map.trigger_zones.iter().enumerate() {
        if hit_tile[0] >= z.min[0]
            && hit_tile[0] <= z.max[0]
            && hit_tile[1] >= z.min[1]
            && hit_tile[1] <= z.max[1]
        {
            return Some(PickedViewportObject::TriggerZone(idx));
        }
    }

    None
}

pub(super) fn clear_all_studio_selections(
    state: &mut StudioState,
    interaction: &mut ClientInteractionState,
) {
    interaction.selected_ids.clear();
    state.selected_prop_ids.clear();
    state.selected_civ_ids.clear();
    state.selected_spawn_players.clear();
    state.selected_zone_indices.clear();
}

pub(super) fn apply_picked_object_to_selection(
    picked: Option<PickedViewportObject>,
    additive_shift: bool,
    state: &mut StudioState,
    interaction: &mut ClientInteractionState,
    world: &SimWorld,
) {
    if !additive_shift {
        clear_all_studio_selections(state, interaction);
    }
    let Some(obj) = picked else {
        return;
    };
    match obj {
        PickedViewportObject::SimEntity(eid) => {
            if additive_shift && interaction.selected_ids.contains(&eid) {
                interaction.selected_ids.remove(&eid);
            } else {
                interaction.selected_ids.insert(eid);
                if let Some(ent) = world.entities.get(&eid) {
                    state.inspect_target = match &ent.kind {
                        EntityKind::Unit { unit_id } => SdkInspectTarget::Unit(unit_id.clone()),
                        EntityKind::Building { building_id } => {
                            SdkInspectTarget::Building(building_id.clone())
                        }
                    };
                }
                focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
            }
        }
        PickedViewportObject::Prop(pid) => {
            if additive_shift && state.selected_prop_ids.contains(&pid) {
                state.selected_prop_ids.remove(&pid);
            } else {
                state.selected_prop_ids.insert(pid);
                focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
            }
        }
        PickedViewportObject::CivilianStructure(cid) => {
            if additive_shift && state.selected_civ_ids.contains(&cid) {
                state.selected_civ_ids.remove(&cid);
            } else {
                state.selected_civ_ids.insert(cid);
                focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
            }
        }
        PickedViewportObject::PlayerSpawn(sp_pid) => {
            if additive_shift && state.selected_spawn_players.contains(&sp_pid) {
                state.selected_spawn_players.remove(&sp_pid);
            } else {
                state.selected_spawn_players.insert(sp_pid);
                focus_or_open_tab(&mut state.dock_state, StudioTab::PropertyInspector);
            }
        }
        PickedViewportObject::TriggerZone(z_idx) => {
            if additive_shift && state.selected_zone_indices.contains(&z_idx) {
                state.selected_zone_indices.remove(&z_idx);
            } else {
                state.selected_zone_indices.insert(z_idx);
                state.selected_zone_idx = z_idx;
                focus_or_open_tab(&mut state.dock_state, StudioTab::TriggerStudio);
            }
        }
    }
}

pub(super) fn draw_height_conforming_circle(
    gizmos: &mut Gizmos,
    grid: &rts_simulation::pathfinding::CollisionGrid,
    center: Vec2,
    radius_world: f32,
    color: Color,
    segments: usize,
) {
    let segs = segments.max(12);
    for i in 0..segs {
        let a0 = (i as f32 / segs as f32) * TAU;
        let a1 = ((i + 1) as f32 / segs as f32) * TAU;
        let x0 = center.x + a0.cos() * radius_world;
        let z0 = center.y + a0.sin() * radius_world;
        let x1 = center.x + a1.cos() * radius_world;
        let z1 = center.y + a1.sin() * radius_world;
        let y0 = grid.sample_world_height(x0, z0) + 0.10;
        let y1 = grid.sample_world_height(x1, z1) + 0.10;
        gizmos.line(Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1), color);
    }
}

pub(super) fn draw_footprint_ghost_at_tile(
    gizmos: &mut Gizmos,
    world: &SimWorld,
    loaded: &LoadedMod,
    state: &StudioState,
    tile: [i32; 2],
    rotated: bool,
) {
    let ts = world.grid.tile_size.max(1.0);
    let (fp, is_naval) = match state.active_tool {
        WorldBuilderTool::PlaceBuilding => {
            let bid = if state.selected_building_brush_id.is_empty() {
                loaded
                    .buildings
                    .keys()
                    .next()
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                state.selected_building_brush_id.as_str()
            };
            if let Some(b) = loaded.buildings.get(bid) {
                let raw = b.footprint;
                let f = if rotated { [raw[1], raw[0]] } else { raw };
                (f, b.water_bound)
            } else {
                ([2, 2], false)
            }
        }
        WorldBuilderTool::PlaceCivilianStructure => {
            let raw = state.civ_footprint;
            (if rotated { [raw[1], raw[0]] } else { raw }, false)
        }
        WorldBuilderTool::PlaceUnit => {
            let uid = if state.selected_unit_brush_id.is_empty() {
                loaded.units.keys().next().map(|s| s.as_str()).unwrap_or("")
            } else {
                state.selected_unit_brush_id.as_str()
            };
            let naval = loaded
                .units
                .get(uid)
                .is_some_and(|u| u.movement_domain == MovementDomain::Naval);
            ([1, 1], naval)
        }
        WorldBuilderTool::PlacePlayerSpawn => ([3, 3], false),
        WorldBuilderTool::PlaceProp => ([1, 1], false),
        WorldBuilderTool::PlaceTriggerZone => {
            let r = state.brush_radius_tiles.max(1) as u32;
            ([r * 2 + 1, r * 2 + 1], false)
        }
        _ => return,
    };

    let base_t = if state.active_tool == WorldBuilderTool::PlaceTriggerZone {
        [
            tile[0] - state.brush_radius_tiles.max(1),
            tile[1] - state.brush_radius_tiles.max(1),
        ]
    } else {
        tile
    };

    let mut all_valid = true;
    for dz in 0..fp[1] as i32 {
        for dx in 0..fp[0] as i32 {
            let t = [base_t[0] + dx, base_t[1] + dz];
            let in_bounds =
                t[0] >= 0 && t[1] >= 0 && t[0] < world.grid.width && t[1] < world.grid.height;
            let blocked = !in_bounds
                || world.grid.blocked_tiles.contains(&t)
                || (if is_naval {
                    !world.grid.water_tiles.contains(&t)
                } else {
                    world.grid.water_tiles.contains(&t) && !world.grid.bridge_tiles.contains(&t)
                });
            let tile_valid = state.active_tool == WorldBuilderTool::PlaceTriggerZone || !blocked;
            if !tile_valid {
                all_valid = false;
            }
            let col = if tile_valid {
                Color::srgba(0.20, 0.96, 0.46, 0.92)
            } else {
                Color::srgba(0.98, 0.24, 0.24, 0.94)
            };
            let x0 = t[0] as f32 * ts;
            let z0 = t[1] as f32 * ts;
            let x1 = (t[0] + 1) as f32 * ts;
            let z1 = (t[1] + 1) as f32 * ts;
            let p00 = Vec3::new(x0, world.grid.sample_world_height(x0, z0) + 0.11, z0);
            let p10 = Vec3::new(x1, world.grid.sample_world_height(x1, z0) + 0.11, z0);
            let p11 = Vec3::new(x1, world.grid.sample_world_height(x1, z1) + 0.11, z1);
            let p01 = Vec3::new(x0, world.grid.sample_world_height(x0, z1) + 0.11, z1);
            gizmos.line(p00, p10, col);
            gizmos.line(p10, p11, col);
            gizmos.line(p11, p01, col);
            gizmos.line(p01, p00, col);
        }
    }

    // Orientation arrow (`[R]` rotates 90 degrees)
    let cx = (base_t[0] as f32 + fp[0] as f32 * 0.5) * ts;
    let cz = (base_t[1] as f32 + fp[1] as f32 * 0.5) * ts;
    let cy = world.grid.sample_world_height(cx, cz) + 0.28;
    let dir = if rotated {
        Vec3::new(1.35, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.35)
    };
    let arrow_col = if all_valid {
        Color::srgb(0.40, 1.0, 0.62)
    } else {
        Color::srgb(1.0, 0.42, 0.38)
    };
    gizmos.arrow(
        Vec3::new(cx, cy, cz),
        Vec3::new(cx, cy, cz) + dir,
        arrow_col,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn studio_3d_ray_picking_respects_locked_and_hidden_entities() {
        let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root")
            .join("rust-ra2");
        let loaded = LoadedMod::load_from_dir(&mod_dir).expect("load rust-ra2");
        let world = SimWorld::from_mod(&loaded);
        let mut state = StudioState::default();

        let (&eid, ent) = world.entities.iter().next().expect("at least one entity");
        let hit = Vec3::new(ent.position[0], 0.0, ent.position[2]);
        let ray = Ray3d::new(hit + Vec3::new(0.0, 20.0, 0.0), Dir3::NEG_Y);

        assert_eq!(
            pick_studio_object_at_ray(ray, hit, &state, &world, &loaded),
            Some(PickedViewportObject::SimEntity(eid))
        );

        state.locked_entity_ids.insert(eid);
        assert_ne!(
            pick_studio_object_at_ray(ray, hit, &state, &world, &loaded),
            Some(PickedViewportObject::SimEntity(eid))
        );
    }
}
